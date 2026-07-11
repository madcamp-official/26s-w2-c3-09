use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::proposal::{Proposal, ProposalReport};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DecisionEntry {
    pub proposal_id: String,
    pub decision: Decision,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Approved,
    Rejected,
}

#[derive(Debug)]
pub enum DecisionError {
    ReadDecision {
        path: String,
        message: String,
    },
    ParseDecision {
        path: String,
        line: usize,
        message: String,
    },
    UnknownProposalId {
        proposal_id: String,
    },
    DuplicateDecision {
        proposal_id: String,
    },
    MissingRejectionReason {
        proposal_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionApplication {
    pub approved: ProposalReport,
    pub rejected: Vec<RejectedProposal>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RejectedProposal {
    pub proposal: Proposal,
    pub reason: String,
}

pub fn read_decision_file(path: impl AsRef<Path>) -> Result<Vec<DecisionEntry>, DecisionError> {
    let path = path.as_ref();
    let content = fs::read_to_string(path).map_err(|error| DecisionError::ReadDecision {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    let content = content.trim_start_matches('\u{feff}');

    content
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            serde_json::from_str::<DecisionEntry>(line).map_err(|error| {
                DecisionError::ParseDecision {
                    path: path.display().to_string(),
                    line: index + 1,
                    message: error.to_string(),
                }
            })
        })
        .collect()
}

pub fn apply_decisions(
    report: ProposalReport,
    decisions: &[DecisionEntry],
) -> Result<DecisionApplication, DecisionError> {
    let known_ids = report
        .proposals
        .iter()
        .map(|proposal| proposal.proposal_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let mut seen_ids = std::collections::HashSet::new();

    for decision in decisions {
        if !known_ids.contains(decision.proposal_id.as_str()) {
            return Err(DecisionError::UnknownProposalId {
                proposal_id: decision.proposal_id.clone(),
            });
        }

        if !seen_ids.insert(decision.proposal_id.as_str()) {
            return Err(DecisionError::DuplicateDecision {
                proposal_id: decision.proposal_id.clone(),
            });
        }

        if decision.decision == Decision::Rejected && rejection_reason(decision).is_none() {
            return Err(DecisionError::MissingRejectionReason {
                proposal_id: decision.proposal_id.clone(),
            });
        }
    }

    let approved_ids = decisions
        .iter()
        .filter(|decision| decision.decision == Decision::Approved)
        .map(|decision| decision.proposal_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let rejected_by_id = decisions
        .iter()
        .filter(|decision| decision.decision == Decision::Rejected)
        .filter_map(|decision| {
            rejection_reason(decision).map(|reason| (decision.proposal_id.as_str(), reason))
        })
        .collect::<std::collections::HashMap<_, _>>();

    let mut approved = Vec::new();
    let mut rejected = Vec::new();

    for proposal in report.proposals {
        if approved_ids.contains(proposal.proposal_id.as_str()) {
            approved.push(proposal);
        } else if let Some(reason) = rejected_by_id.get(proposal.proposal_id.as_str()) {
            rejected.push(RejectedProposal {
                proposal,
                reason: (*reason).to_string(),
            });
        }
    }

    Ok(DecisionApplication {
        approved: ProposalReport {
            root: report.root,
            proposals: approved,
        },
        rejected,
    })
}

fn rejection_reason(decision: &DecisionEntry) -> Option<&str> {
    decision
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
}

impl fmt::Display for DecisionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecisionError::ReadDecision { path, message } => {
                write!(formatter, "cannot read decision file {path}: {message}")
            }
            DecisionError::ParseDecision {
                path,
                line,
                message,
            } => {
                write!(
                    formatter,
                    "cannot parse decision file {path} line {line}: {message}"
                )
            }
            DecisionError::UnknownProposalId { proposal_id } => {
                write!(
                    formatter,
                    "decision references unknown proposal_id {proposal_id}"
                )
            }
            DecisionError::DuplicateDecision { proposal_id } => {
                write!(
                    formatter,
                    "decision contains duplicate proposal_id {proposal_id}"
                )
            }
            DecisionError::MissingRejectionReason { proposal_id } => {
                write!(
                    formatter,
                    "rejected decision for proposal_id {proposal_id} must include a non-empty reason"
                )
            }
        }
    }
}

impl Error for DecisionError {}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::proposal::propose_for_root;

    use super::{apply_decisions, read_decision_file, Decision, DecisionEntry};

    #[test]
    fn reads_decision_jsonl() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("decision.jsonl");
        fs::write(
            &path,
            r#"{"proposal_id":"move:inbox/note.md:documents/note.md","decision":"approved"}"#,
        )
        .expect("write decision");

        let decisions = read_decision_file(&path).expect("read decision");

        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].decision, Decision::Approved);
        assert_eq!(decisions[0].reason, None);
    }

    #[test]
    fn keeps_only_approved_proposals() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        fs::write(root.join("inbox").join("photo.png"), "png").expect("write photo");
        let report = propose_for_root(&root).expect("propose");

        let approved = vec![DecisionEntry {
            proposal_id: report.proposals[0].proposal_id.clone(),
            decision: Decision::Approved,
            reason: None,
        }];

        let filtered = apply_decisions(report, &approved).expect("apply decisions");

        assert_eq!(filtered.approved.proposals.len(), 1);
        assert_eq!(filtered.approved.proposals[0].from, "inbox/note.md");
    }

    #[test]
    fn rejects_unknown_proposal_id() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        let report = propose_for_root(&root).expect("propose");

        let decisions = vec![DecisionEntry {
            proposal_id: "move:missing.txt:documents/missing.txt".to_string(),
            decision: Decision::Approved,
            reason: None,
        }];

        let error = apply_decisions(report, &decisions).expect_err("unknown id");

        assert!(error.to_string().contains("unknown proposal_id"));
    }

    #[test]
    fn rejects_duplicate_decision() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        let report = propose_for_root(&root).expect("propose");

        let note_id = report.proposals[0].proposal_id.clone();
        let decisions = vec![
            DecisionEntry {
                proposal_id: note_id.clone(),
                decision: Decision::Approved,
                reason: None,
            },
            DecisionEntry {
                proposal_id: note_id,
                decision: Decision::Rejected,
                reason: Some("manual skip".to_string()),
            },
        ];

        let error = apply_decisions(report, &decisions).expect_err("duplicate id");

        assert!(error.to_string().contains("duplicate proposal_id"));
    }

    #[test]
    fn requires_reason_for_rejected_decision() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        let report = propose_for_root(&root).expect("propose");

        let decisions = vec![DecisionEntry {
            proposal_id: report.proposals[0].proposal_id.clone(),
            decision: Decision::Rejected,
            reason: None,
        }];

        let error = apply_decisions(report, &decisions).expect_err("missing reason");

        assert!(error.to_string().contains("non-empty reason"));
    }

    #[test]
    fn returns_rejected_proposals_with_reason() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        let report = propose_for_root(&root).expect("propose");

        let decisions = vec![DecisionEntry {
            proposal_id: report.proposals[0].proposal_id.clone(),
            decision: Decision::Rejected,
            reason: Some("keep it in inbox".to_string()),
        }];

        let application = apply_decisions(report, &decisions).expect("apply decisions");

        assert!(application.approved.proposals.is_empty());
        assert_eq!(application.rejected.len(), 1);
        assert_eq!(application.rejected[0].reason, "keep it in inbox");
    }
}
