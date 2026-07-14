use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::decision::{Decision, DecisionEntry};
use crate::proposal::{ProposalAction, ProposalReport, ProposalStatus};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AutoApprovalPolicy {
    pub enabled: bool,
    pub allowed_actions: Vec<ProposalAction>,
    pub max_files_per_run: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub enum AutoApprovalError {
    Disabled,
    EmptyAllowedActions,
    MaxFilesPerRunZero,
    TooManyReadyProposals { ready: usize, max: usize },
}

pub fn auto_approve_decisions(
    report: &ProposalReport,
    policy: &AutoApprovalPolicy,
) -> Result<Vec<DecisionEntry>, AutoApprovalError> {
    policy.validate()?;

    let allowed = policy
        .allowed_actions
        .iter()
        .collect::<HashSet<&ProposalAction>>();
    let ready_allowed = report
        .proposals
        .iter()
        .filter(|proposal| {
            proposal.status == ProposalStatus::Ready && allowed.contains(&proposal.action)
        })
        .collect::<Vec<_>>();

    if ready_allowed.len() > policy.max_files_per_run {
        return Err(AutoApprovalError::TooManyReadyProposals {
            ready: ready_allowed.len(),
            max: policy.max_files_per_run,
        });
    }

    Ok(ready_allowed
        .into_iter()
        .map(|proposal| DecisionEntry {
            proposal_id: proposal.proposal_id.clone(),
            decision: Decision::Approved,
            reason: None,
        })
        .collect())
}

impl AutoApprovalPolicy {
    pub fn validate(&self) -> Result<(), AutoApprovalError> {
        if !self.enabled {
            return Err(AutoApprovalError::Disabled);
        }
        if self.allowed_actions.is_empty() {
            return Err(AutoApprovalError::EmptyAllowedActions);
        }
        if self.max_files_per_run == 0 {
            return Err(AutoApprovalError::MaxFilesPerRunZero);
        }

        Ok(())
    }
}

impl fmt::Display for AutoApprovalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AutoApprovalError::Disabled => write!(formatter, "auto approval policy is disabled"),
            AutoApprovalError::EmptyAllowedActions => {
                write!(formatter, "auto approval policy has no allowed actions")
            }
            AutoApprovalError::MaxFilesPerRunZero => {
                write!(
                    formatter,
                    "auto approval max_files_per_run must be greater than zero"
                )
            }
            AutoApprovalError::TooManyReadyProposals { ready, max } => {
                write!(
                    formatter,
                    "auto approval would approve {ready} proposals, exceeding limit {max}"
                )
            }
        }
    }
}

impl Error for AutoApprovalError {}

#[cfg(test)]
mod tests {
    use crate::proposal::{Proposal, ProposalAction, ProposalReport, ProposalStatus};

    use super::{auto_approve_decisions, AutoApprovalError, AutoApprovalPolicy};

    fn proposal(id: &str, action: ProposalAction, status: ProposalStatus) -> Proposal {
        Proposal {
            proposal_id: id.to_string(),
            action,
            from: "inbox/file.tmp".to_string(),
            to: ".mousekeeper_trash".to_string(),
            content: None,
            source_size_bytes: 1,
            source_modified_unix_ms: None,
            source_file_id: None,
            reason: "test".to_string(),
            status,
        }
    }

    #[test]
    fn approves_only_ready_allowed_actions() {
        let report = ProposalReport {
            root: "root".to_string(),
            proposals: vec![
                proposal("trash:1", ProposalAction::Trash, ProposalStatus::Ready),
                proposal("move:1", ProposalAction::Move, ProposalStatus::Ready),
                proposal(
                    "trash:2",
                    ProposalAction::Trash,
                    ProposalStatus::DestinationExists,
                ),
            ],
        };
        let policy = AutoApprovalPolicy {
            enabled: true,
            allowed_actions: vec![ProposalAction::Trash],
            max_files_per_run: 2,
        };

        let decisions = auto_approve_decisions(&report, &policy).expect("auto approve");

        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].proposal_id, "trash:1");
    }

    #[test]
    fn refuses_to_exceed_run_limit() {
        let report = ProposalReport {
            root: "root".to_string(),
            proposals: vec![
                proposal("trash:1", ProposalAction::Trash, ProposalStatus::Ready),
                proposal("trash:2", ProposalAction::Trash, ProposalStatus::Ready),
            ],
        };
        let policy = AutoApprovalPolicy {
            enabled: true,
            allowed_actions: vec![ProposalAction::Trash],
            max_files_per_run: 1,
        };

        let error = auto_approve_decisions(&report, &policy).expect_err("limit");

        assert_eq!(
            error,
            AutoApprovalError::TooManyReadyProposals { ready: 2, max: 1 }
        );
    }
}
