use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use file_engine_cli::fs_safety::is_link_or_reparse_point;
use file_engine_cli::path_guard::{PathGuard, PathGuardError};

use crate::agent::{
    AgentFileTransfer, AgentFileTransferFailureCode, AgentFileTransferSourceVersion, AgentRuntime,
};
use crate::storage::managed_roots::ManagedRootStore;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedTransferSource {
    pub transfer_id: String,
    pub room_id: String,
    pub source_relative_path: String,
    pub resolved_path: PathBuf,
    pub source_version: AgentFileTransferSourceVersion,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferSourceValidationError {
    pub failure_code: AgentFileTransferFailureCode,
    pub message: String,
}

pub async fn validate_transfer_source_for_request(
    agent: &AgentRuntime,
    roots: &ManagedRootStore,
    transfer: &AgentFileTransfer,
) -> Result<ValidatedTransferSource, TransferSourceValidationError> {
    let room = agent
        .root_id_for_room(transfer.room_id.clone())
        .await
        .map_err(|error| TransferSourceValidationError {
            failure_code: AgentFileTransferFailureCode::OutsideManagedRoot,
            message: error.to_string(),
        })?;
    let managed_root = roots
        .get(&room.root_id)
        .map_err(|error| TransferSourceValidationError {
            failure_code: AgentFileTransferFailureCode::OutsideManagedRoot,
            message: error,
        })?;
    if !managed_root.enabled {
        return Err(TransferSourceValidationError {
            failure_code: AgentFileTransferFailureCode::OutsideManagedRoot,
            message: format!("managed root is disabled: {}", managed_root.root_id),
        });
    }

    let mut validated = validate_local_transfer_source(
        &managed_root.root,
        &transfer.source_relative_path,
        Some(&transfer.transfer_id),
        Some(&transfer.room_id),
    )?;
    validated.transfer_id = transfer.transfer_id.clone();
    validated.room_id = transfer.room_id.clone();
    Ok(validated)
}

pub fn validate_local_transfer_source(
    root: &str,
    source_relative_path: &str,
    transfer_id: Option<&str>,
    room_id: Option<&str>,
) -> Result<ValidatedTransferSource, TransferSourceValidationError> {
    validate_relative_path_shape(source_relative_path)?;
    let guard = PathGuard::new(root).map_err(map_guard_error)?;
    let resolved = guard
        .resolve_existing(source_relative_path)
        .map_err(map_guard_error)?;

    let original_entry = guard.root().join(source_relative_path);
    let original_metadata =
        fs::symlink_metadata(&original_entry).map_err(|error| TransferSourceValidationError {
            failure_code: AgentFileTransferFailureCode::SourceNotFound,
            message: format!("cannot read source metadata: {error}"),
        })?;
    let original_type = original_metadata.file_type();
    if is_link_or_reparse_point(&original_metadata, original_type) {
        return Err(TransferSourceValidationError {
            failure_code: AgentFileTransferFailureCode::OutsideManagedRoot,
            message: "source file is a symlink, junction, or reparse point".to_string(),
        });
    }

    let metadata = fs::metadata(&resolved).map_err(|error| TransferSourceValidationError {
        failure_code: AgentFileTransferFailureCode::SourceNotFound,
        message: format!("cannot read source metadata: {error}"),
    })?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(TransferSourceValidationError {
            failure_code: AgentFileTransferFailureCode::SourceNotFound,
            message: "source path is not a transferable non-empty file".to_string(),
        });
    }

    let modified_unix_ms = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis());
    let source_version = AgentFileTransferSourceVersion {
        file_id: stable_file_id(source_relative_path, metadata.len(), modified_unix_ms),
        size_bytes: metadata.len(),
        modified_at: unix_ms_to_rfc3339(modified_unix_ms),
    };

    Ok(ValidatedTransferSource {
        transfer_id: transfer_id.unwrap_or_default().to_string(),
        room_id: room_id.unwrap_or_default().to_string(),
        source_relative_path: normalize_relative_path(source_relative_path),
        resolved_path: resolved,
        source_version,
    })
}

fn validate_relative_path_shape(path: &str) -> Result<(), TransferSourceValidationError> {
    if path.is_empty()
        || path.len() > 1024
        || path.contains('\0')
        || path.starts_with('/')
        || path.starts_with('\\')
        || looks_like_windows_absolute(path)
    {
        return Err(outside_root("source path must be managed-root-relative"));
    }
    if path
        .split(['/', '\\'])
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(outside_root(
            "source path cannot contain empty, current, or parent segments",
        ));
    }
    Ok(())
}

fn looks_like_windows_absolute(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn normalize_relative_path(path: &str) -> String {
    path.split(['/', '\\']).collect::<Vec<_>>().join("/")
}

fn map_guard_error(error: PathGuardError) -> TransferSourceValidationError {
    let failure_code = match error {
        PathGuardError::MissingPath(_) => AgentFileTransferFailureCode::SourceNotFound,
        PathGuardError::RootNotDirectory(_)
        | PathGuardError::AbsoluteInput(_)
        | PathGuardError::ParentTraversal(_)
        | PathGuardError::EscapesRoot { .. }
        | PathGuardError::Io(_) => AgentFileTransferFailureCode::OutsideManagedRoot,
    };
    TransferSourceValidationError {
        failure_code,
        message: error.to_string(),
    }
}

fn outside_root(message: &str) -> TransferSourceValidationError {
    TransferSourceValidationError {
        failure_code: AgentFileTransferFailureCode::OutsideManagedRoot,
        message: message.to_string(),
    }
}

fn stable_file_id(relative_path: &str, size_bytes: u64, modified_unix_ms: Option<u128>) -> String {
    let modified = modified_unix_ms
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    format!(
        "hm:{:016x}",
        fnv1a_64(
            format!(
                "{}|{}|{}|{}",
                normalize_relative_path(relative_path),
                false,
                size_bytes,
                modified
            )
            .as_bytes()
        )
    )
}

fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn unix_ms_to_rfc3339(unix_ms: Option<u128>) -> String {
    let millis = unix_ms.unwrap_or(0);
    let seconds = i64::try_from(millis / 1000).unwrap_or(i64::MAX);
    let sub_ms = u32::try_from(millis % 1000).unwrap_or(0);
    let (year, month, day, hour, minute, second) = unix_seconds_to_utc(seconds);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{sub_ms:03}Z")
}

fn unix_seconds_to_utc(seconds: i64) -> (i32, u32, u32, u32, u32, u32) {
    let days = seconds.div_euclid(86_400);
    let day_seconds = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = u32::try_from(day_seconds / 3_600).unwrap_or(0);
    let minute = u32::try_from((day_seconds % 3_600) / 60).unwrap_or(0);
    let second = u32::try_from(day_seconds % 60).unwrap_or(0);
    (year, month, day, hour, minute, second)
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (
        i32::try_from(year).unwrap_or(i32::MAX),
        u32::try_from(month).unwrap_or(1),
        u32::try_from(day).unwrap_or(1),
    )
}

#[allow(dead_code)]
fn unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{unix_ms_to_rfc3339, validate_local_transfer_source};
    use crate::agent::AgentFileTransferFailureCode;

    #[test]
    fn validates_existing_file_and_builds_source_version() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("docs")).expect("docs");
        fs::write(root.join("docs").join("report.pdf"), "hello").expect("file");

        let source = validate_local_transfer_source(
            root.to_str().expect("root"),
            "docs/report.pdf",
            Some("transfer-1"),
            Some("room-1"),
        )
        .expect("valid source");

        assert_eq!(source.transfer_id, "transfer-1");
        assert_eq!(source.room_id, "room-1");
        assert_eq!(source.source_relative_path, "docs/report.pdf");
        assert_eq!(source.source_version.size_bytes, 5);
        assert!(source.source_version.file_id.starts_with("hm:"));
        assert!(source.resolved_path.ends_with("report.pdf"));
    }

    #[test]
    fn rejects_traversal_before_source_resolution() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("root");

        let error = validate_local_transfer_source(
            root.to_str().expect("root"),
            "../secret.txt",
            None,
            None,
        )
        .expect_err("traversal rejected");

        assert_eq!(
            error.failure_code,
            AgentFileTransferFailureCode::OutsideManagedRoot
        );
    }

    #[test]
    fn rejects_missing_source_as_source_not_found() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("root");

        let error =
            validate_local_transfer_source(root.to_str().expect("root"), "missing.pdf", None, None)
                .expect_err("missing rejected");

        assert_eq!(
            error.failure_code,
            AgentFileTransferFailureCode::SourceNotFound
        );
    }

    #[test]
    fn rejects_directory_source() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("docs")).expect("docs");

        let error =
            validate_local_transfer_source(root.to_str().expect("root"), "docs", None, None)
                .expect_err("directory rejected");

        assert_eq!(
            error.failure_code,
            AgentFileTransferFailureCode::SourceNotFound
        );
    }

    #[test]
    fn rejects_empty_file_because_server_source_version_requires_positive_size() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("root");
        fs::write(root.join("empty.txt"), "").expect("empty file");

        let error =
            validate_local_transfer_source(root.to_str().expect("root"), "empty.txt", None, None)
                .expect_err("empty file rejected");

        assert_eq!(
            error.failure_code,
            AgentFileTransferFailureCode::SourceNotFound
        );
    }

    #[test]
    fn rejects_absolute_path_shape() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("root");

        let error = validate_local_transfer_source(
            root.to_str().expect("root"),
            "C:/Users/user/secret.txt",
            None,
            None,
        )
        .expect_err("absolute rejected");

        assert_eq!(
            error.failure_code,
            AgentFileTransferFailureCode::OutsideManagedRoot
        );
    }

    #[test]
    fn unix_ms_format_is_contract_datetime() {
        assert_eq!(
            unix_ms_to_rfc3339(Some(1_704_067_200_123)),
            "2024-01-01T00:00:00.123Z"
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_source_even_when_target_stays_inside_root() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("root");
        fs::write(root.join("real.txt"), "real").expect("real file");
        std::os::unix::fs::symlink(root.join("real.txt"), root.join("link.txt")).expect("symlink");

        let error =
            validate_local_transfer_source(root.to_str().expect("root"), "link.txt", None, None)
                .expect_err("symlink rejected");

        assert_eq!(
            error.failure_code,
            AgentFileTransferFailureCode::OutsideManagedRoot
        );
    }
}
