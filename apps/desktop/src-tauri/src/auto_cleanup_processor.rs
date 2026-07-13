use file_engine_cli::auto_approval::auto_approve_decisions;
use file_engine_cli::decision::apply_decisions;
use file_engine_cli::execute::execute_decision_application;
use file_engine_cli::proposal::{propose_for_root, ProposalReport};
use serde::Serialize;

use crate::storage::auto_approval::AutoApprovalStore;
use crate::storage::managed_roots::ManagedRootStore;

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
    pub executed_count: usize,
    pub skipped_count: usize,
}

/// Runs the local autonomous cleanup pass. Every enabled root gets a proposal when the rule engine
/// finds work, and roots where the user explicitly enabled auto approval also execute the policy
/// approved subset through the normal precheck, journal, and execute path.
pub fn process_auto_cleanup(
    roots: &ManagedRootStore,
    policies: &AutoApprovalStore,
) -> Result<AutoCleanupReport, String> {
    let mut report = AutoCleanupReport::default();

    for root in roots.list()? {
        report.inspected_count += 1;
        if !root.enabled {
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
        let root_report = if policy_record.enabled {
            report.eligible_root_count += 1;
            match process_root_auto_cleanup(
                &root.root,
                proposal.clone(),
                policy_record.to_engine_policy(),
            ) {
                Ok(root_report) => {
                    report.approved_count += root_report.approved_count;
                    report.executed_count += root_report.executed_count;
                    report.skipped_count += root_report.skipped_count;
                    root_report
                }
                Err(_) => {
                    report.failed_count += 1;
                    RootAutoCleanupReport::default()
                }
            }
        } else {
            RootAutoCleanupReport::default()
        };

        report.proposals.push(AutoCleanupProposalEvent {
            root_id: root.root_id,
            proposal,
            auto_approved_count: root_report.approved_count,
            executed_count: root_report.executed_count,
            skipped_count: root_report.skipped_count,
        });
    }

    Ok(report)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct RootAutoCleanupReport {
    approved_count: usize,
    executed_count: usize,
    skipped_count: usize,
}

fn process_root_auto_cleanup(
    root: &str,
    proposal: ProposalReport,
    policy: file_engine_cli::auto_approval::AutoApprovalPolicy,
) -> Result<RootAutoCleanupReport, String> {
    let decisions =
        auto_approve_decisions(&proposal, &policy).map_err(|error| error.to_string())?;
    if decisions.is_empty() {
        return Ok(RootAutoCleanupReport::default());
    }

    let approved_count = decisions.len();
    let application = apply_decisions(proposal, &decisions).map_err(|error| error.to_string())?;
    let execution =
        execute_decision_application(root, application).map_err(|error| error.to_string())?;

    Ok(RootAutoCleanupReport {
        approved_count,
        executed_count: execution.executed_count,
        skipped_count: execution.skipped_count,
    })
}

#[cfg(test)]
mod tests {
    use file_engine_cli::proposal::ProposalAction;
    use tempfile::tempdir;

    use crate::storage::auto_approval::{AutoApprovalPolicyPatch, AutoApprovalStore};
    use crate::storage::managed_roots::{ManagedRoot, ManagedRootStatus, ManagedRootStore};

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
    fn enabled_policy_executes_ready_cleanup() {
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
        assert_eq!(report.executed_count, 1);
        assert_eq!(report.proposals.len(), 1);
        assert!(!temp.path().join("old.tmp").exists());
    }
}
