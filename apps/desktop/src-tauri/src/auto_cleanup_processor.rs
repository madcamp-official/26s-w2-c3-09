use file_engine_cli::auto_approval::auto_approve_decisions;
use file_engine_cli::proposal::{propose_for_root, ProposalReport};
use serde::Serialize;

use crate::storage::auto_approval::AutoApprovalStore;
use crate::storage::managed_roots::{ManagedRootStore, RoomBindingStatus};

pub const AUTO_CLEANUP_PROPOSAL_EVENT: &str = "auto-cleanup-proposal";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AutoCleanupReport {
    pub inspected_count: usize,
    pub proposed_root_count: usize,
    pub proposal_count: usize,
    pub eligible_root_count: usize,
    pub approved_count: usize,
    pub executed_count: usize,
    pub skipped_count: usize,
    pub failed_count: usize,
    pub proposals: Vec<AutoCleanupProposalEvent>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct AutoCleanupProposalEvent {
    pub root_id: String,
    pub proposal: ProposalReport,
    pub auto_approved_count: usize,
    pub auto_approved_proposal_ids: Vec<String>,
    pub executed_count: usize,
    pub skipped_count: usize,
}

/// Runs a proposal-only background scan. It never mutates files: even policy-matched items still
/// require the normal explicit precheck and execute action. Only roots with a durable active mobile
/// binding participate; unbound/detached roots remain available for explicit local actions only.
pub fn process_auto_cleanup(
    roots: &ManagedRootStore,
    policies: &AutoApprovalStore,
) -> Result<AutoCleanupReport, String> {
    let mut report = AutoCleanupReport::default();

    for root in roots.list()? {
        report.inspected_count += 1;
        if !root.enabled || root.room_binding_status != RoomBindingStatus::Active {
            continue;
        }

        let proposal = match propose_for_root(&root.root).map_err(|error| error.to_string()) {
            Ok(proposal) => proposal,
            Err(_) => {
                report.failed_count += 1;
                continue;
            }
        };
        if proposal.proposals.is_empty() {
            continue;
        }

        report.proposed_root_count += 1;
        report.proposal_count += proposal.proposals.len();

        let policy_record = policies.get_or_default(&root.root_id)?;
        let engine_policy = policy_record.to_engine_policy();
        let auto_approved_proposal_ids = if engine_policy.enabled {
            report.eligible_root_count += 1;
            match auto_approve_decisions(&proposal, &engine_policy) {
                Ok(decisions) => {
                    report.approved_count += decisions.len();
                    decisions
                        .into_iter()
                        .map(|decision| decision.proposal_id)
                        .collect()
                }
                Err(_) => {
                    report.failed_count += 1;
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

        report.proposals.push(AutoCleanupProposalEvent {
            root_id: root.root_id,
            proposal,
            auto_approved_count: auto_approved_proposal_ids.len(),
            auto_approved_proposal_ids,
            executed_count: 0,
            skipped_count: 0,
        });
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use file_engine_cli::proposal::ProposalAction;
    use tempfile::tempdir;

    use crate::storage::auto_approval::{AutoApprovalPolicyPatch, AutoApprovalStore};
    use crate::storage::managed_roots::{
        ManagedRoot, ManagedRootStatus, ManagedRootStore, RoomBindingStatus,
    };

    use super::process_auto_cleanup;

    fn managed_root(root_id: &str, root: String) -> ManagedRoot {
        ManagedRoot {
            root_id: root_id.to_string(),
            root,
            display_name: "Test Root".to_string(),
            enabled: true,
            watch_on_startup: false,
            last_seen_status: ManagedRootStatus::Ready,
            last_error: None,
            registered_unix_ms: 1,
            updated_unix_ms: 1,
            room_id: Some(format!("room-{root_id}")),
            detached_room_id: None,
            room_binding_status: RoomBindingStatus::Active,
        }
    }

    #[test]
    fn disabled_policy_does_not_execute_cleanup() {
        let temp = tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join(".mousekeeper")).expect("state dir");
        std::fs::write(
            temp.path().join(".mousekeeper").join("rules.json"),
            r#"{"version":1,"rules":[{"id":"temp-trash","when":{"name_matches":"*.tmp"},"then":{"trash":true}}]}"#,
        )
        .expect("rules");
        std::fs::write(temp.path().join("old.tmp"), "temporary").expect("fixture");

        let roots = ManagedRootStore::default();
        let managed = roots
            .upsert(managed_root(
                "root:test",
                temp.path()
                    .canonicalize()
                    .expect("canonical")
                    .display()
                    .to_string(),
            ))
            .expect("managed root");
        let policies = AutoApprovalStore::default();

        let report = process_auto_cleanup(&roots, &policies).expect("auto cleanup");

        assert_eq!(report.inspected_count, 1);
        assert_eq!(report.proposed_root_count, 1);
        assert_eq!(report.proposal_count, 1);
        assert_eq!(report.proposals.len(), 1);
        assert_eq!(report.eligible_root_count, 0);
        assert_eq!(report.executed_count, 0);
        assert!(temp.path().join("old.tmp").exists());
        assert_eq!(managed.enabled, true);
    }

    #[test]
    fn enabled_policy_marks_ready_cleanup_without_executing_it() {
        let temp = tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join(".mousekeeper")).expect("state dir");
        std::fs::write(
            temp.path().join(".mousekeeper").join("rules.json"),
            r#"{"version":1,"rules":[{"id":"temp-trash","when":{"name_matches":"*.tmp"},"then":{"trash":true}}]}"#,
        )
        .expect("rules");
        std::fs::write(temp.path().join("old.tmp"), "temporary").expect("fixture");

        let roots = ManagedRootStore::default();
        let managed = roots
            .upsert(managed_root(
                "root:test",
                temp.path()
                    .canonicalize()
                    .expect("canonical")
                    .display()
                    .to_string(),
            ))
            .expect("managed root");
        let policies = AutoApprovalStore::default();
        policies
            .patch(
                &managed.root_id,
                AutoApprovalPolicyPatch {
                    enabled: Some(true),
                    allowed_actions: Some(vec![ProposalAction::Trash]),
                    max_files_per_run: Some(5),
                    expires_unix_ms: None,
                },
            )
            .expect("policy");

        let report = process_auto_cleanup(&roots, &policies).expect("auto cleanup");

        assert_eq!(report.inspected_count, 1);
        assert_eq!(report.proposed_root_count, 1);
        assert_eq!(report.eligible_root_count, 1);
        assert_eq!(report.proposal_count, 1);
        assert_eq!(report.approved_count, 1);
        assert_eq!(report.executed_count, 0);
        assert_eq!(report.proposals.len(), 1);
        assert!(temp.path().join("old.tmp").exists());
    }

    #[test]
    fn expired_policy_is_not_counted_as_effectively_enabled() {
        let temp = tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join(".mousekeeper")).expect("state dir");
        std::fs::write(
            temp.path().join(".mousekeeper").join("rules.json"),
            r#"{"version":1,"rules":[{"id":"temp-trash","when":{"name_matches":"*.tmp"},"then":{"trash":true}}]}"#,
        )
        .expect("rules");
        std::fs::write(temp.path().join("old.tmp"), "temporary").expect("fixture");

        let roots = ManagedRootStore::default();
        let managed = roots
            .upsert(managed_root(
                "root:expired",
                temp.path()
                    .canonicalize()
                    .expect("canonical")
                    .display()
                    .to_string(),
            ))
            .expect("managed root");
        let policies = AutoApprovalStore::default();
        policies
            .patch(
                &managed.root_id,
                AutoApprovalPolicyPatch {
                    enabled: Some(true),
                    allowed_actions: Some(vec![ProposalAction::Trash]),
                    max_files_per_run: Some(5),
                    expires_unix_ms: Some(1),
                },
            )
            .expect("expired policy");

        let report = process_auto_cleanup(&roots, &policies).expect("scan");

        assert_eq!(report.proposed_root_count, 1);
        assert_eq!(report.eligible_root_count, 0);
        assert_eq!(report.approved_count, 0);
        assert_eq!(report.executed_count, 0);
        assert!(temp.path().join("old.tmp").exists());
    }

    #[test]
    fn detached_root_is_not_proposed_or_executed() {
        let temp = tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join(".mousekeeper")).expect("state dir");
        std::fs::write(
            temp.path().join(".mousekeeper").join("rules.json"),
            r#"{"version":1,"rules":[{"id":"temp-trash","when":{"name_matches":"*.tmp"},"then":{"trash":true}}]}"#,
        )
        .expect("rules");
        std::fs::write(temp.path().join("old.tmp"), "temporary").expect("fixture");

        let roots = ManagedRootStore::default();
        let mut detached = managed_root(
            "root:detached",
            temp.path()
                .canonicalize()
                .expect("canonical")
                .display()
                .to_string(),
        );
        detached.room_id = None;
        detached.room_binding_status = RoomBindingStatus::Detached;
        detached.detached_room_id = Some("room-detached".to_string());
        roots.upsert(detached).expect("managed root");

        let report = process_auto_cleanup(&roots, &AutoApprovalStore::default()).expect("scan");

        assert_eq!(report.inspected_count, 1);
        assert_eq!(report.proposed_root_count, 0);
        assert_eq!(report.executed_count, 0);
        assert!(report.proposals.is_empty());
        assert!(temp.path().join("old.tmp").exists());
    }

    #[test]
    fn unbound_root_is_not_submitted_by_background_scan() {
        let temp = tempdir().expect("tempdir");
        let roots = ManagedRootStore::default();
        let mut unbound = managed_root(
            "root:unbound",
            temp.path()
                .canonicalize()
                .expect("canonical")
                .display()
                .to_string(),
        );
        unbound.room_id = None;
        unbound.room_binding_status = RoomBindingStatus::Unbound;
        roots.upsert(unbound).expect("managed root");

        let report = process_auto_cleanup(&roots, &AutoApprovalStore::default()).expect("scan");

        assert_eq!(report.inspected_count, 1);
        assert_eq!(report.proposed_root_count, 0);
        assert!(report.proposals.is_empty());
    }
}
