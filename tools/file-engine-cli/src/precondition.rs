use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use serde::Serialize;

use crate::path_guard::{PathGuard, PathGuardError};
use crate::proposal::{propose_for_root, Proposal, ProposalError, ProposalReport, ProposalStatus};

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct PrecheckReport {
    pub root: String,
    pub checks: Vec<PrecheckResult>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct PrecheckResult {
    pub from: String,
    pub to: String,
    pub status: PrecheckStatus,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PrecheckStatus {
    Ready,
    DestinationExists,
    MissingSource,
    SourceChanged,
    RejectedPath,
}

#[derive(Debug)]
pub enum PrecheckError {
    Proposal(ProposalError),
    Guard(PathGuardError),
    RootMismatch { expected: String, actual: String },
    Serialize(String),
}

pub fn precheck_root(root: impl AsRef<Path>) -> Result<PrecheckReport, PrecheckError> {
    let guard = PathGuard::new(root).map_err(PrecheckError::Guard)?;
    let report = propose_for_root(guard.root()).map_err(PrecheckError::Proposal)?;

    precheck_proposals_with_guard(guard, report)
}

pub fn precheck_proposals(
    root: impl AsRef<Path>,
    report: ProposalReport,
) -> Result<PrecheckReport, PrecheckError> {
    let guard = PathGuard::new(root).map_err(PrecheckError::Guard)?;

    precheck_proposals_with_guard(guard, report)
}

fn precheck_proposals_with_guard(
    guard: PathGuard,
    report: ProposalReport,
) -> Result<PrecheckReport, PrecheckError> {
    let expected_root = guard.root().display().to_string();
    if report.root != expected_root {
        return Err(PrecheckError::RootMismatch {
            expected: expected_root,
            actual: report.root,
        });
    }

    let checks = report
        .proposals
        .iter()
        .map(|proposal| precheck_proposal(&guard, proposal))
        .collect::<Vec<_>>();

    Ok(PrecheckReport {
        root: report.root,
        checks,
    })
}

fn precheck_proposal(guard: &PathGuard, proposal: &Proposal) -> PrecheckResult {
    if proposal.status == ProposalStatus::DestinationExists {
        return result(
            proposal,
            PrecheckStatus::DestinationExists,
            Some("destination already exists; execution must not overwrite it".to_string()),
        );
    }

    let source = match guard.resolve_existing(&proposal.from) {
        Ok(source) => source,
        Err(PathGuardError::MissingPath(_)) => {
            return result(
                proposal,
                PrecheckStatus::MissingSource,
                Some("source file no longer exists".to_string()),
            );
        }
        Err(error) => {
            return result(
                proposal,
                PrecheckStatus::RejectedPath,
                Some(error.to_string()),
            );
        }
    };

    let metadata = match fs::metadata(&source) {
        Ok(metadata) => metadata,
        Err(error) => {
            return result(
                proposal,
                PrecheckStatus::MissingSource,
                Some(error.to_string()),
            );
        }
    };

    let current_size = metadata.len();
    let current_modified = modified_unix_ms(&metadata);

    if current_size != proposal.source_size_bytes
        || current_modified != proposal.source_modified_unix_ms
    {
        return result(
            proposal,
            PrecheckStatus::SourceChanged,
            Some("source size or modified time changed since proposal".to_string()),
        );
    }

    result(proposal, PrecheckStatus::Ready, None)
}

fn result(proposal: &Proposal, status: PrecheckStatus, reason: Option<String>) -> PrecheckResult {
    PrecheckResult {
        from: proposal.from.clone(),
        to: proposal.to.clone(),
        status,
        reason,
    }
}

fn modified_unix_ms(metadata: &fs::Metadata) -> Option<u128> {
    metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis())
}

impl fmt::Display for PrecheckError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrecheckError::Proposal(error) => write!(formatter, "{error}"),
            PrecheckError::Guard(error) => write!(formatter, "{error}"),
            PrecheckError::RootMismatch { expected, actual } => {
                write!(
                    formatter,
                    "proposal root mismatch: expected {expected}, got {actual}"
                )
            }
            PrecheckError::Serialize(message) => {
                write!(formatter, "cannot serialize precheck report: {message}")
            }
        }
    }
}

impl Error for PrecheckError {}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::proposal::propose_for_root;

    use super::{precheck_proposals, precheck_root, PrecheckStatus};

    #[test]
    fn reports_ready_for_unchanged_source() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");

        let report = precheck_root(&root).expect("precheck");

        assert_eq!(report.checks.len(), 1);
        assert_eq!(report.checks[0].status, PrecheckStatus::Ready);
    }

    #[test]
    fn blocks_existing_destination() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::create_dir_all(root.join("documents")).expect("create documents");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        fs::write(root.join("documents").join("note.md"), "# existing").expect("write existing");

        let report = precheck_root(&root).expect("precheck");

        assert_eq!(report.checks.len(), 1);
        assert_eq!(report.checks[0].status, PrecheckStatus::DestinationExists);
    }

    #[test]
    fn detects_source_change_from_saved_proposal() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        let note = root.join("inbox").join("note.md");
        fs::write(&note, "# note").expect("write note");
        let proposal = propose_for_root(&root).expect("propose");

        fs::write(&note, "# changed note").expect("modify note");

        let report = precheck_proposals(&root, proposal).expect("precheck saved proposal");

        assert_eq!(report.checks.len(), 1);
        assert_eq!(report.checks[0].status, PrecheckStatus::SourceChanged);
    }
}
