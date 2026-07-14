use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::analyzer::{analyze_root, AnalyzeError, AnalyzeReport};
use crate::rules::{
    load_rule_set_for_root, normalize_relative_path, RuleContext, RuleError, RuleSet,
};

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    pub source_size_bytes: u64,
    pub source_modified_unix_ms: Option<u128>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_file_id: Option<String>,
    pub reason: String,
    pub status: ProposalStatus,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ProposalAction {
    Move,
    Trash,
    CreateDir,
    CreateFile,
    ReadmeWrite,
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
    Rule(RuleError),
    ReadProposal { path: String, message: String },
    ParseProposal { path: String, message: String },
    Serialize(String),
}

pub fn propose_for_root(root: impl AsRef<Path>) -> Result<ProposalReport, ProposalError> {
    let root = root.as_ref();
    let rule_set = load_rule_set_for_root(root).map_err(ProposalError::Rule)?;
    propose_for_root_with_rule_set(root, rule_set)
}

/// Builds proposals from a scan the caller already completed. Cleanliness snapshots use this
/// path so their total-file metrics and proposal deductions always describe one filesystem view.
pub fn propose_for_analysis(
    root: impl AsRef<Path>,
    report: &AnalyzeReport,
) -> Result<ProposalReport, ProposalError> {
    let rule_set = load_rule_set_for_root(root).map_err(ProposalError::Rule)?;
    propose_for_analysis_with_rule_set(report, rule_set)
}

/// Computes a proposal by applying a caller-supplied rule set to the managed root, rather than the
/// root's persisted `rules.json`. This is the deterministic engine an AI/rule-draft flow feeds
/// into: the draft is validated (`RuleSet::validate`) before it reaches this function, and the
/// engine — not the AI — computes the concrete file targets. The rule set is validated again here
/// so no caller can hand in an unvalidated set.
pub fn propose_for_root_with_rule_set(
    root: impl AsRef<Path>,
    rule_set: RuleSet,
) -> Result<ProposalReport, ProposalError> {
    let root = root.as_ref();
    let report = analyze_root(root).map_err(ProposalError::Analyze)?;
    propose_for_analysis_with_rule_set(&report, rule_set)
}

fn propose_for_analysis_with_rule_set(
    report: &AnalyzeReport,
    rule_set: RuleSet,
) -> Result<ProposalReport, ProposalError> {
    rule_set.validate().map_err(ProposalError::Rule)?;
    let mut existing_paths = report
        .files
        .iter()
        .map(|file| normalize_relative_path(&file.path))
        .collect::<HashSet<_>>();
    existing_paths.extend(
        report
            .directories
            .iter()
            .map(|directory| normalize_relative_path(directory)),
    );
    let context = RuleContext {
        existing_paths,
        now_unix_ms: unix_ms(),
    };

    let mut seen_proposal_ids = HashSet::new();
    let proposals = report
        .files
        .iter()
        .filter_map(|file| rule_set.propose(file, &context))
        .filter(|proposal| seen_proposal_ids.insert(proposal.proposal_id.clone()))
        .collect::<Vec<_>>();

    Ok(ProposalReport {
        root: report.root.clone(),
        proposals,
    })
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

pub fn proposal_id(action: &ProposalAction, from: &str, to: &str) -> String {
    let input = format!(
        "{}|{}|{}",
        action.as_str(),
        normalize_id_part(from),
        normalize_id_part(to)
    );

    format!("{}:{:016x}", action.as_str(), fnv1a64(input.as_bytes()))
}

fn normalize_id_part(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325;

    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }

    hash
}

impl ProposalAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProposalAction::Move => "move",
            ProposalAction::Trash => "trash",
            ProposalAction::CreateDir => "create_dir",
            ProposalAction::CreateFile => "create_file",
            ProposalAction::ReadmeWrite => "readme_write",
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
            ProposalError::Rule(error) => write!(formatter, "{error}"),
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

    use super::{
        propose_for_root, propose_for_root_with_rule_set, read_proposal_file, ProposalAction,
        ProposalStatus,
    };

    #[test]
    fn applies_a_supplied_rule_set_deterministically_without_touching_files() {
        use crate::rules::RuleSet;

        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("downloads")).expect("create downloads");
        // Two PDFs plus an unrelated file; the draft rule should only match the PDFs.
        fs::write(root.join("downloads").join("old.pdf"), "pdf").expect("write pdf");
        fs::write(root.join("downloads").join("report.pdf"), "pdf").expect("write pdf");
        fs::write(root.join("downloads").join("keep.txt"), "txt").expect("write txt");

        // The shape an AI "clean up old PDFs" draft would produce (age omitted here so the test is
        // deterministic; the engine still computes the concrete targets).
        let draft = r#"{"version":1,"rules":[
            {"id":"cleanup-pdfs","when":{"extension_in":["pdf"]},"then":{"trash":true}}
        ]}"#;
        let rule_set = serde_json::from_str::<RuleSet>(draft).expect("parse draft");

        let report = propose_for_root_with_rule_set(&root, rule_set).expect("propose");

        let mut froms = report
            .proposals
            .iter()
            .map(|proposal| proposal.from.as_str())
            .collect::<Vec<_>>();
        froms.sort();
        assert_eq!(froms, vec!["downloads/old.pdf", "downloads/report.pdf"]);
        // Deterministic proposal only — nothing on disk changed.
        assert!(root.join("downloads").join("old.pdf").exists());
        assert!(root.join("downloads").join("keep.txt").exists());
    }

    #[test]
    fn refuses_to_apply_an_invalid_rule_set() {
        use crate::rules::RuleSet;

        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("create root");

        // A rule with no condition would mean "match everything" — validation must reject it
        // before any file access.
        let invalid = RuleSet {
            version: 1,
            rules: vec![crate::rules::Rule {
                id: "empty".to_string(),
                priority: 0,
                when: crate::rules::Condition::default(),
                then: crate::rules::Action {
                    move_to: None,
                    trash: true,
                    create_dir: None,
                },
            }],
        };

        assert!(propose_for_root_with_rule_set(&root, invalid).is_err());
    }

    #[test]
    fn create_dir_contract_rule_produces_one_deduplicated_proposal() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("downloads")).expect("create downloads");
        fs::write(root.join("downloads").join("old.pdf"), "pdf").expect("write pdf");
        fs::write(root.join("downloads").join("report.pdf"), "pdf").expect("write pdf");

        let rule_set = crate::rules::RuleSet::from_contract_definition(
            "create-archive",
            0,
            serde_json::json!({
                "match": "ALL",
                "conditions": [
                    {"field": "extension", "operator": "IN", "value": [".pdf"]}
                ],
                "action": {"type": "CREATE_DIR", "relativePath": "Archive/PDF"}
            }),
        )
        .expect("contract rule");

        let report = propose_for_root_with_rule_set(&root, rule_set).expect("propose");

        assert_eq!(report.proposals.len(), 1);
        assert_eq!(report.proposals[0].action, ProposalAction::CreateDir);
        assert_eq!(report.proposals[0].to, "Archive/PDF");
        assert_eq!(report.proposals[0].status, ProposalStatus::Ready);
    }

    #[test]
    fn create_dir_contract_rule_detects_existing_directory() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("downloads")).expect("create downloads");
        fs::create_dir_all(root.join("Archive").join("PDF")).expect("create existing archive");
        fs::write(root.join("downloads").join("old.pdf"), "pdf").expect("write pdf");

        let rule_set = crate::rules::RuleSet::from_contract_definition(
            "create-archive",
            0,
            serde_json::json!({
                "match": "ALL",
                "conditions": [
                    {"field": "extension", "operator": "IN", "value": [".pdf"]}
                ],
                "action": {"type": "CREATE_DIR", "relativePath": "Archive/PDF"}
            }),
        )
        .expect("contract rule");

        let report = propose_for_root_with_rule_set(&root, rule_set).expect("propose");

        assert_eq!(report.proposals.len(), 1);
        assert_eq!(
            report.proposals[0].status,
            ProposalStatus::DestinationExists
        );
    }

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
        assert!(report.proposals[0].proposal_id.starts_with("move:"));
        assert_eq!(report.proposals[0].proposal_id.len(), 21);
        assert!(report.proposals[0].source_file_id.is_some());
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
