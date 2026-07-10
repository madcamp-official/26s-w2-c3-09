use std::error::Error;
use std::fmt;
use std::path::Path;

use serde::Serialize;

use crate::analyzer::{analyze_root, AnalyzeError};
use crate::rules::{default_rules, normalize_relative_path, RuleContext};

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ProposalReport {
    pub root: String,
    pub proposals: Vec<Proposal>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Proposal {
    pub action: ProposalAction,
    pub from: String,
    pub to: String,
    pub source_size_bytes: u64,
    pub source_modified_unix_ms: Option<u128>,
    pub reason: String,
    pub status: ProposalStatus,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProposalAction {
    Move,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    Ready,
    DestinationExists,
}

#[derive(Debug)]
pub enum ProposalError {
    Analyze(AnalyzeError),
    Serialize(String),
}

pub fn propose_for_root(root: impl AsRef<Path>) -> Result<ProposalReport, ProposalError> {
    let report = analyze_root(root).map_err(ProposalError::Analyze)?;
    let existing_paths = report
        .files
        .iter()
        .map(|file| normalize_relative_path(&file.path))
        .collect();
    let context = RuleContext { existing_paths };
    let rules = default_rules();

    let proposals = report
        .files
        .iter()
        .filter_map(|file| rules.iter().find_map(|rule| rule.propose(file, &context)))
        .collect::<Vec<_>>();

    Ok(ProposalReport {
        root: report.root,
        proposals,
    })
}

impl fmt::Display for ProposalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProposalError::Analyze(error) => write!(formatter, "{error}"),
            ProposalError::Serialize(message) => {
                write!(formatter, "cannot serialize proposal report: {message}")
            }
        }
    }
}

impl Error for ProposalError {}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{propose_for_root, ProposalStatus};

    #[test]
    fn proposes_moves_by_extension_without_touching_files() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        fs::write(root.join("inbox").join("photo.png"), "png").expect("write photo");
        fs::write(root.join("inbox").join("unknown.bin"), "bin").expect("write unknown");

        let report = propose_for_root(&root).expect("propose");

        let moves = report
            .proposals
            .iter()
            .map(|proposal| {
                (
                    proposal.from.as_str(),
                    proposal.to.as_str(),
                    &proposal.status,
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            moves,
            vec![
                ("inbox/note.md", "documents/note.md", &ProposalStatus::Ready),
                (
                    "inbox/photo.png",
                    "images/photo.png",
                    &ProposalStatus::Ready
                ),
            ]
        );
        assert!(root.join("inbox").join("note.md").exists());
        assert!(root.join("inbox").join("photo.png").exists());
    }

    #[test]
    fn marks_existing_destination_as_collision() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::create_dir_all(root.join("documents")).expect("create documents");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        fs::write(root.join("documents").join("note.md"), "# existing").expect("write existing");

        let report = propose_for_root(&root).expect("propose");

        assert_eq!(report.proposals.len(), 1);
        assert_eq!(
            report.proposals[0].status,
            ProposalStatus::DestinationExists
        );
    }
}
