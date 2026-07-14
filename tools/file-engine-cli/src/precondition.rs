use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use serde::Serialize;

use crate::decision::DecisionError;
use crate::fs_safety::is_link_or_reparse_point;
use crate::path_guard::{PathGuard, PathGuardError};
use crate::proposal::{
    propose_for_root, Proposal, ProposalAction, ProposalError, ProposalReport, ProposalStatus,
};

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct PrecheckReport {
    pub root: String,
    pub checks: Vec<PrecheckResult>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct PrecheckResult {
    pub action: ProposalAction,
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
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
    Decision(DecisionError),
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

    match proposal.action {
        ProposalAction::CreateDir => return precheck_create_dir(guard, proposal),
        ProposalAction::CreateFile => return precheck_create_file(guard, proposal),
        ProposalAction::ReadmeWrite => return precheck_readme_write(guard, proposal),
        ProposalAction::Move | ProposalAction::Trash => {}
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
    if let Some(reason) = source_identity_change_reason(&source, proposal) {
        return result(proposal, PrecheckStatus::SourceChanged, Some(reason));
    }

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

fn precheck_create_dir(guard: &PathGuard, proposal: &Proposal) -> PrecheckResult {
    if proposal.to.is_empty() || proposal.to == "." || proposal.to == ".." {
        return result(
            proposal,
            PrecheckStatus::RejectedPath,
            Some("CREATE_DIR target must be a relative directory path".to_string()),
        );
    }

    let target = match guard.resolve_for_create(&proposal.to) {
        Ok(target) => target,
        Err(error) => {
            return result(
                proposal,
                PrecheckStatus::RejectedPath,
                Some(error.to_string()),
            );
        }
    };

    if target.exists() {
        return result(
            proposal,
            PrecheckStatus::DestinationExists,
            Some("directory already exists; execution must not overwrite it".to_string()),
        );
    }

    result(proposal, PrecheckStatus::Ready, None)
}

fn precheck_create_file(guard: &PathGuard, proposal: &Proposal) -> PrecheckResult {
    if proposal.to.is_empty() || proposal.to == "." || proposal.to == ".." {
        return result(
            proposal,
            PrecheckStatus::RejectedPath,
            Some("CREATE_FILE target must be a relative file path".to_string()),
        );
    }
    if proposal.content.as_deref().unwrap_or_default() != "" {
        return result(
            proposal,
            PrecheckStatus::RejectedPath,
            Some("CREATE_FILE currently supports empty files only".to_string()),
        );
    }

    let target = match guard.resolve_for_create(&proposal.to) {
        Ok(target) => target,
        Err(error) => {
            return result(
                proposal,
                PrecheckStatus::RejectedPath,
                Some(error.to_string()),
            );
        }
    };

    if target.exists() {
        return result(
            proposal,
            PrecheckStatus::DestinationExists,
            Some("file already exists; execution must not overwrite it".to_string()),
        );
    }

    result(proposal, PrecheckStatus::Ready, None)
}

fn precheck_readme_write(guard: &PathGuard, proposal: &Proposal) -> PrecheckResult {
    if proposal.from != "README.md" || proposal.to != "README.md" {
        return result(
            proposal,
            PrecheckStatus::RejectedPath,
            Some("README_WRITE may only target README.md at the managed-root root".to_string()),
        );
    }
    if proposal.content.is_none() {
        return result(
            proposal,
            PrecheckStatus::RejectedPath,
            Some("README_WRITE proposal is missing approved content".to_string()),
        );
    }

    let raw_target = guard.root().join("README.md");
    let target_metadata = fs::symlink_metadata(&raw_target).ok();
    if let Some(metadata) = &target_metadata {
        if is_link_or_reparse_point(metadata, metadata.file_type()) {
            return result(
                proposal,
                PrecheckStatus::RejectedPath,
                Some("README.md is a symlink, junction, or reparse point".to_string()),
            );
        }
    }

    let target = match guard.resolve_existing("README.md") {
        Ok(target) => target,
        Err(PathGuardError::MissingPath(_)) => {
            if proposal.source_size_bytes == 0
                && proposal.source_modified_unix_ms.is_none()
                && proposal.source_file_id.is_none()
            {
                return result(proposal, PrecheckStatus::Ready, None);
            }
            return result(
                proposal,
                PrecheckStatus::MissingSource,
                Some("README.md no longer exists".to_string()),
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

    let metadata = match fs::metadata(&target) {
        Ok(metadata) => metadata,
        Err(error) => {
            return result(
                proposal,
                PrecheckStatus::MissingSource,
                Some(error.to_string()),
            );
        }
    };

    if let Some(reason) = source_identity_change_reason(&target, proposal) {
        return result(proposal, PrecheckStatus::SourceChanged, Some(reason));
    }

    if metadata.len() != proposal.source_size_bytes
        || modified_unix_ms(&metadata) != proposal.source_modified_unix_ms
    {
        return result(
            proposal,
            PrecheckStatus::SourceChanged,
            Some("README.md changed since proposal".to_string()),
        );
    }

    result(proposal, PrecheckStatus::Ready, None)
}

fn source_identity_change_reason(path: &Path, proposal: &Proposal) -> Option<String> {
    let expected = proposal.source_file_id.as_ref()?;
    match crate::file_identity::file_id_for_path(path) {
        Some(current) if current == *expected => None,
        Some(_) => Some("source file identity changed since proposal".to_string()),
        None => Some("source file identity is unavailable during precheck".to_string()),
    }
}

fn result(proposal: &Proposal, status: PrecheckStatus, reason: Option<String>) -> PrecheckResult {
    PrecheckResult {
        action: proposal.action.clone(),
        from: proposal.from.clone(),
        to: proposal.to.clone(),
        content: proposal.content.clone(),
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
            PrecheckError::Decision(error) => write!(formatter, "{error}"),
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

    #[test]
    fn detects_source_identity_change_even_when_size_and_mtime_match() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        let mut proposal = propose_for_root(&root).expect("propose");
        proposal.proposals[0].source_file_id =
            Some("test:identity-that-does-not-match-current-file".to_string());

        let report = precheck_proposals(&root, proposal).expect("precheck saved proposal");

        assert_eq!(report.checks.len(), 1);
        assert_eq!(report.checks[0].status, PrecheckStatus::SourceChanged);
        assert_eq!(
            report.checks[0].reason.as_deref(),
            Some("source file identity changed since proposal")
        );
    }
}
