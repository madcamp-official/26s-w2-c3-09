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
//! - Roots whose auto-approval policy is enabled clean themselves up locally, so their proposals are
//!   never also pushed as pending approvals.
//! - A room that already has an open proposal is skipped, so the 15s background tick submits at most
//!   one autonomous proposal per room at a time.

use std::time::{SystemTime, UNIX_EPOCH};

use file_engine_cli::proposal::ProposalReport;

use crate::agent::AgentRuntime;
use crate::auto_cleanup_processor::AutoCleanupReport;
use crate::command_processor::build_agent_proposal_submission;
use crate::storage::auto_approval::AutoApprovalStore;
use crate::storage::managed_roots::ManagedRootStore;

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

/// True when a root's freshly computed cleanup should be pushed to mobile as a pending proposal.
/// Auto-approving roots handle their own cleanup locally, and a root with nothing to propose has
/// nothing to send.
fn should_submit_to_mobile(policy_enabled: bool, proposal_len: usize) -> bool {
    !policy_enabled && proposal_len > 0
}

/// Submits the eligible proposals from an auto-cleanup pass to the server. Best-effort by design:
/// each root is independent, and any failure is counted rather than aborting the rest.
pub async fn submit_autonomous_proposals(
    agent: &AgentRuntime,
    roots: &ManagedRootStore,
    auto_approval: &AutoApprovalStore,
    cleanup: &AutoCleanupReport,
) -> AutoProposalReport {
    let mut report = AutoProposalReport::default();

    for event in &cleanup.proposals {
        let policy_enabled = auto_approval
            .get_or_default(&event.root_id)
            .map(|policy| policy.enabled)
            .unwrap_or(false);
        if !should_submit_to_mobile(policy_enabled, event.proposal.proposals.len()) {
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
    let room = agent
        .ensure_room_for_root(managed.root_id.clone(), managed.display_name.clone())
        .await
        .map_err(|error| error.to_string())?;

    // Dedup: never stack a second autonomous proposal on a room that still has one open.
    if agent
        .open_proposal_count_for_room(room.room_id.clone())
        .await
        .map_err(|error| error.to_string())?
        > 0
    {
        return Ok(false);
    }

    let stamp = unix_ms();
    let command = agent
        .create_command(
            room.room_id.clone(),
            AUTO_PROPOSAL_INTENT.to_string(),
            serde_json::json!({}),
            format!("autocmd-{root_id}-{stamp}"),
        )
        .await
        .map_err(|error| error.to_string())?;
    // An idempotent replay hands back a command that is no longer QUEUED; its proposal already
    // exists, so there is nothing more to submit.
    if command.status != "QUEUED" {
        return Ok(false);
    }

    agent
        .update_command_status(command.command_id.clone(), "ANALYZING".to_string())
        .await
        .map_err(|error| error.to_string())?;

    let max = proposal.proposals.len().clamp(1, MAX_PROPOSAL_ITEMS);
    let submission = build_agent_proposal_submission(&command, proposal, max)?;
    agent
        .submit_proposal(format!("autoprop-{root_id}-{stamp}"), submission)
        .await
        .map_err(|error| error.to_string())?;

    Ok(true)
}

fn unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::should_submit_to_mobile;

    #[test]
    fn submits_only_non_auto_roots_with_work() {
        // Manual (policy-disabled) root with proposals -> push to mobile for approval.
        assert!(should_submit_to_mobile(false, 3));
        // Nothing to propose -> nothing to send.
        assert!(!should_submit_to_mobile(false, 0));
        // Auto-approving root cleans itself up locally -> never also a pending mobile approval.
        assert!(!should_submit_to_mobile(true, 3));
        assert!(!should_submit_to_mobile(true, 0));
    }
}
