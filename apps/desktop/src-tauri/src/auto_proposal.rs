//! Pushes the desktop's *autonomous* cleanup proposals to the server so they appear on mobile as
//! pending approvals, without the user having to request them from the phone.
//!
//! The server models every proposal as the result of a command: a proposal can only be created
//! against a command that is already `ANALYZING` (see the server's proposals service). So to surface
//! a cleanup the desktop discovered on its own, this module synthesizes the missing command — it
//! creates an `ANALYZE` command in the room, moves it to `ANALYZING`, then submits the proposal
//! against it. From that point the normal decision/execution pipeline takes over.
//!
//! Two invariants keep this from spamming mobile:
//! - A room that already has an open proposal is skipped, so the 15s background tick submits at most
//!   one autonomous proposal per room at a time.
//! - Command and proposal idempotency keys are derived from the proposal contents, so a timeout can
//!   resume the same server workflow instead of accumulating orphan commands.

use file_engine_cli::proposal::ProposalReport;
use sha2::{Digest, Sha256};

use crate::agent::AgentRuntime;
use crate::auto_cleanup_processor::AutoCleanupReport;
use crate::command_processor::build_agent_proposal_submission;
use crate::storage::managed_roots::{ManagedRootStore, RoomBindingStatus};

const AUTO_PROPOSAL_INTENT: &str = "ANALYZE";
const MAX_PROPOSAL_ITEMS: usize = 200;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AutoProposalReport {
    /// Roots that produced proposals and were eligible for a mobile submission this pass.
    pub considered_count: usize,
    /// New autonomous proposals submitted to the server this pass.
    pub submitted_count: usize,
    /// Roots skipped because the room already had an open proposal (or an idempotent replay).
    pub skipped_count: usize,
    /// Roots where the submission failed (offline, transient server error, ...).
    pub failed_count: usize,
}

/// A background scan is proposal-only, so every non-empty proposal goes through mobile approval.
fn should_submit_to_mobile(proposal_len: usize) -> bool {
    proposal_len > 0
}

/// Submits the eligible proposals from an auto-cleanup pass to the server. Best-effort by design:
/// each root is independent, and any failure is counted rather than aborting the rest.
pub async fn submit_autonomous_proposals(
    agent: &AgentRuntime,
    roots: &ManagedRootStore,
    cleanup: &AutoCleanupReport,
) -> AutoProposalReport {
    let mut report = AutoProposalReport::default();

    for event in &cleanup.proposals {
        if !should_submit_to_mobile(event.proposal.proposals.len()) {
            continue;
        }

        report.considered_count += 1;
        match submit_one(agent, roots, &event.root_id, event.proposal.clone()).await {
            Ok(true) => report.submitted_count += 1,
            Ok(false) => report.skipped_count += 1,
            Err(_) => report.failed_count += 1,
        }
    }

    report
}

/// Submits a single root's proposal. Returns `Ok(true)` when a new proposal was created, `Ok(false)`
/// when it was intentionally skipped (a proposal is already open, or the command was an idempotent
/// replay), and `Err` when a step failed.
async fn submit_one(
    agent: &AgentRuntime,
    roots: &ManagedRootStore,
    root_id: &str,
    proposal: ProposalReport,
) -> Result<bool, String> {
    let managed = roots.get(root_id)?;
    if managed.room_binding_status != RoomBindingStatus::Active {
        return Ok(false);
    }
    let room_id = managed
        .room_id
        .clone()
        .ok_or_else(|| "active managed root is missing its durable room id".to_string())?;

    // Dedup: never stack a second autonomous proposal on a room that still has one open.
    if agent
        .open_proposal_count_for_room(room_id.clone())
        .await
        .map_err(|error| error.to_string())?
        > 0
    {
        return Ok(false);
    }

    let fingerprint = proposal_fingerprint(root_id, &proposal)?;
    let command = agent
        .create_command(
            room_id,
            AUTO_PROPOSAL_INTENT.to_string(),
            serde_json::json!({}),
            format!("autocmd-{root_id}-{fingerprint}"),
        )
        .await
        .map_err(|error| error.to_string())?;
    match command.status.as_str() {
        "QUEUED" => {
            agent
                .update_command_status(command.command_id.clone(), "ANALYZING".to_string())
                .await
                .map_err(|error| error.to_string())?;
        }
        // A previous status update may have committed before its response was lost. Resume with
        // the same proposal key instead of creating a second command.
        "ANALYZING" => {}
        "WAITING_APPROVAL"
        | "APPROVED"
        | "REJECTED"
        | "EXECUTING"
        | "SUCCEEDED"
        | "PARTIALLY_SUCCEEDED"
        | "FAILED"
        | "EXPIRED"
        | "STALE" => return Ok(false),
        status => {
            return Err(format!(
                "autonomous proposal command cannot resume from status {status}"
            ));
        }
    }

    let max = proposal.proposals.len().clamp(1, MAX_PROPOSAL_ITEMS);
    let mut submission = build_agent_proposal_submission(&command, proposal, max)?;
    // The shared builder records wall-clock check time for interactive requests. Autonomous retry
    // keys are content-derived, so remove that volatile metadata to keep the idempotent request body
    // byte-for-byte equivalent across timeout retries. Source versions remain in every item.
    for item in &mut submission.items {
        if let Some(precondition) = item.precondition.as_object_mut() {
            precondition.remove("checkedAtUnixMs");
        }
    }
    agent
        .submit_proposal(format!("autoprop-{root_id}-{fingerprint}"), submission)
        .await
        .map_err(|error| error.to_string())?;

    Ok(true)
}

fn proposal_fingerprint(root_id: &str, proposal: &ProposalReport) -> Result<String, String> {
    let encoded = serde_json::to_vec(proposal)
        .map_err(|error| format!("cannot fingerprint autonomous proposal: {error}"))?;
    let mut hasher = Sha256::new();
    hasher.update(root_id.as_bytes());
    hasher.update([0]);
    hasher.update(encoded);
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

#[cfg(test)]
mod tests {
    use file_engine_cli::proposal::{Proposal, ProposalAction, ProposalReport, ProposalStatus};

    use super::{proposal_fingerprint, should_submit_to_mobile};

    #[test]
    fn submits_every_non_empty_proposal() {
        // Nothing to propose -> nothing to send.
        assert!(!should_submit_to_mobile(0));
        // Background scans never execute, so every concrete proposal needs mobile approval.
        assert!(should_submit_to_mobile(3));
    }

    #[test]
    fn proposal_fingerprint_is_stable_and_changes_with_source_version() {
        let report = ProposalReport {
            root: "C:/managed".to_string(),
            proposals: vec![Proposal {
                proposal_id: "move:one".to_string(),
                action: ProposalAction::Move,
                from: "inbox/a.txt".to_string(),
                to: "docs/a.txt".to_string(),
                content: None,
                source_size_bytes: 10,
                source_modified_unix_ms: Some(20),
                reason: "rule".to_string(),
                status: ProposalStatus::Ready,
            }],
        };
        let same = proposal_fingerprint("root-1", &report).expect("fingerprint");
        assert_eq!(
            same,
            proposal_fingerprint("root-1", &report).expect("stable")
        );

        let mut changed = report.clone();
        changed.proposals[0].source_modified_unix_ms = Some(21);
        assert_ne!(
            same,
            proposal_fingerprint("root-1", &changed).expect("changed fingerprint")
        );
    }
}
