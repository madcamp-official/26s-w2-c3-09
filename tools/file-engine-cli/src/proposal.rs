use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::analyzer::{analyze_root, AnalyzeError};
use crate::rules::{default_rules, normalize_relative_path, RuleContext};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProposalReport {
    pub root: String,
    pub proposals: Vec<Proposal>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Proposal {
    pub proposal_id: String,
    pub action: ProposalAction,
    pub from: String,
    pub to: String,
    pub source_size_bytes: u64,
    pub source_modified_unix_ms: Option<u128>,
    pub reason: String,
    pub status: ProposalStatus,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProposalAction {
    Move,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    Ready,
    DestinationExists,
}

#[derive(Debug)]
pub enum ProposalError {
    Analyze(AnalyzeError),
    ReadProposal { path: String, message: String },
    ParseProposal { path: String, message: String },
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

pub fn proposal_id(action: &ProposalAction, from: &str, to: &str) -> String {
    format!(
        "{}:{}:{}",
        action.as_str(),
        normalize_id_part(from),
        normalize_id_part(to)
    )
}

fn normalize_id_part(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

impl ProposalAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProposalAction::Move => "move",
        }
    }
}

pub fn read_proposal_file(path: impl AsRef<Path>) -> Result<ProposalReport, ProposalError> {
    let path = path.as_ref();
    let content = fs::read_to_string(path).map_err(|error| ProposalError::ReadProposal {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;

    let content = content.trim_start_matches('\u{feff}');

    serde_json::from_str(content).map_err(|error| ProposalError::ParseProposal {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

impl fmt::Display for ProposalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProposalError::Analyze(error) => write!(formatter, "{error}"),
            ProposalError::ReadProposal { path, message } => {
                write!(formatter, "cannot read proposal file {path}: {message}")
            }
            ProposalError::ParseProposal { path, message } => {
                write!(formatter, "cannot parse proposal file {path}: {message}")
            }
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

    use super::{propose_for_root, read_proposal_file, ProposalStatus};

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
        assert_eq!(
            report.proposals[0].proposal_id,
            "move:inbox/note.md:documents/note.md"
        );
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

    #[test]
    fn proposal_report_round_trips_through_json() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");

        let report = propose_for_root(&root).expect("propose");
        let json = serde_json::to_string(&report).expect("serialize");
        let decoded = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(report, decoded);
    }

    #[test]
    fn reads_proposal_report_from_json_file() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        let proposal_path = temp.path().join("proposal.json");

        let report = propose_for_root(&root).expect("propose");
        fs::write(
            &proposal_path,
            serde_json::to_string_pretty(&report).expect("serialize"),
        )
        .expect("write proposal");

        let decoded = read_proposal_file(&proposal_path).expect("read proposal");

        assert_eq!(report, decoded);
    }

    #[test]
    fn reads_proposal_report_with_utf8_bom() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        let proposal_path = temp.path().join("proposal.json");

        let report = propose_for_root(&root).expect("propose");
        fs::write(
            &proposal_path,
            format!(
                "\u{feff}{}",
                serde_json::to_string_pretty(&report).expect("serialize")
            ),
        )
        .expect("write proposal");

        let decoded = read_proposal_file(&proposal_path).expect("read proposal");

        assert_eq!(report, decoded);
    }
}
