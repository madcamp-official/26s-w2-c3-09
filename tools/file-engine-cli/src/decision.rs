use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::proposal::ProposalReport;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DecisionEntry {
    pub proposal_id: String,
    pub decision: Decision,
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

pub fn apply_decisions(report: ProposalReport, decisions: &[DecisionEntry]) -> ProposalReport {
    let approved_ids = decisions
        .iter()
        .filter(|decision| decision.decision == Decision::Approved)
        .map(|decision| decision.proposal_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let proposals = report
        .proposals
        .into_iter()
        .filter(|proposal| approved_ids.contains(proposal.proposal_id.as_str()))
        .collect();

    ProposalReport {
        root: report.root,
        proposals,
    }
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
            proposal_id: "move:inbox/note.md:documents/note.md".to_string(),
            decision: Decision::Approved,
        }];

        let filtered = apply_decisions(report, &approved);

        assert_eq!(filtered.proposals.len(), 1);
        assert_eq!(filtered.proposals[0].from, "inbox/note.md");
    }
}
