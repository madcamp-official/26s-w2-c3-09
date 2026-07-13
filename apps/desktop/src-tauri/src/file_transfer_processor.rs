use std::fs::{self, File};
use std::io::Read;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use file_engine_cli::fs_safety::is_link_or_reparse_point;
use file_engine_cli::path_guard::{PathGuard, PathGuardError};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::agent::{
    AgentFileTransfer, AgentFileTransferFailureCode, AgentFileTransferSourceVersion,
    AgentFileTransferUploadTarget, AgentRuntime,
};
use crate::outbox_processor::{enqueue_file_transfer_completion, file_transfer_completion_key};
use crate::storage::managed_roots::ManagedRootStore;
use crate::storage::outbox::OutboxStore;

const FILE_READ_CHUNK_BYTES: usize = 1024 * 1024;
const UPLOAD_TIMEOUT_SECONDS: u64 = 120;

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedTransferUpload {
    pub source: ValidatedTransferSource,
    pub upload_target: AgentFileTransferUploadTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferUploadPreparationError {
    pub transfer_id: String,
    pub failure_code: Option<AgentFileTransferFailureCode>,
    pub failure_reported: bool,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletedTransferUpload {
    pub transfer_id: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferUploadExecutionError {
    pub transfer_id: String,
    pub failure_code: Option<AgentFileTransferFailureCode>,
    pub failure_reported: bool,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct FileTransferProcessingReport {
    pub inspected_count: usize,
    pub uploaded_count: usize,
    pub failed_count: usize,
    pub skipped_count: usize,
    pub results: Vec<FileTransferProcessingResult>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct FileTransferProcessingResult {
    pub transfer_id: String,
    pub status: FileTransferProcessingStatus,
    pub size_bytes: Option<u64>,
    pub sha256: Option<String>,
    pub failure_code: Option<AgentFileTransferFailureCode>,
    pub failure_reported: bool,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileTransferProcessingStatus {
    Completed,
    Failed,
    Skipped,
}

pub async fn process_pending_file_transfers(
    agent: &AgentRuntime,
    roots: &ManagedRootStore,
    outbox: &OutboxStore,
) -> Result<FileTransferProcessingReport, String> {
    let transfers = agent
        .pending_file_transfers()
        .await
        .map_err(|error| error.to_string())?;

    let mut report = FileTransferProcessingReport {
        inspected_count: transfers.len(),
        uploaded_count: 0,
        failed_count: 0,
        skipped_count: 0,
        results: Vec::new(),
    };

    for transfer in transfers {
        let transfer_id = transfer.transfer_id.clone();
        match process_one_transfer(agent, roots, outbox, transfer).await {
            Ok(completed) => {
                report.uploaded_count += 1;
                report.results.push(FileTransferProcessingResult {
                    transfer_id,
                    status: FileTransferProcessingStatus::Completed,
                    size_bytes: Some(completed.size_bytes),
                    sha256: Some(completed.sha256),
                    failure_code: None,
                    failure_reported: false,
                    message: None,
                });
            }
            Err(error) if error.message.contains("not pending") => {
                report.skipped_count += 1;
                report.results.push(FileTransferProcessingResult {
                    transfer_id,
                    status: FileTransferProcessingStatus::Skipped,
                    size_bytes: None,
                    sha256: None,
                    failure_code: error.failure_code,
                    failure_reported: error.failure_reported,
                    message: Some(error.message),
                });
            }
            Err(error) => {
                report.failed_count += 1;
                report.results.push(FileTransferProcessingResult {
                    transfer_id,
                    status: FileTransferProcessingStatus::Failed,
                    size_bytes: None,
                    sha256: None,
                    failure_code: error.failure_code,
                    failure_reported: error.failure_reported,
                    message: Some(error.message),
                });
            }
        }
    }

    Ok(report)
}

async fn process_one_transfer(
    agent: &AgentRuntime,
    roots: &ManagedRootStore,
    outbox: &OutboxStore,
    transfer: AgentFileTransfer,
) -> Result<CompletedTransferUpload, TransferUploadExecutionError> {
    let prepared = prepare_upload_target_for_transfer(agent, roots, &transfer)
        .await
        .map_err(|error| TransferUploadExecutionError {
            transfer_id: error.transfer_id,
            failure_code: error.failure_code,
            failure_reported: error.failure_reported,
            message: error.message,
        })?;
    upload_prepared_transfer(agent, outbox, prepared).await
}

pub async fn prepare_upload_target_for_transfer(
    agent: &AgentRuntime,
    roots: &ManagedRootStore,
    transfer: &AgentFileTransfer,
) -> Result<PreparedTransferUpload, TransferUploadPreparationError> {
    if transfer.status != "REQUESTED" {
        return Err(TransferUploadPreparationError {
            transfer_id: transfer.transfer_id.clone(),
            failure_code: None,
            failure_reported: false,
            message: format!("file transfer is not pending: {}", transfer.status),
        });
    }

    let source = match validate_transfer_source_for_request(agent, roots, transfer).await {
        Ok(source) => source,
        Err(error) => {
            let failure_reported = agent
                .fail_file_transfer(transfer.transfer_id.clone(), error.failure_code.clone())
                .await
                .is_ok();
            return Err(TransferUploadPreparationError {
                transfer_id: transfer.transfer_id.clone(),
                failure_code: Some(error.failure_code),
                failure_reported,
                message: error.message,
            });
        }
    };

    let upload_target = match agent
        .request_file_transfer_upload_target(
            transfer.transfer_id.clone(),
            source.source_version.clone(),
        )
        .await
    {
        Ok(upload_target) => upload_target,
        Err(error) => {
            let failure_code = upload_target_failure_code(&error.message);
            let failure_reported = match failure_code.clone() {
                Some(code) => agent
                    .fail_file_transfer(transfer.transfer_id.clone(), code)
                    .await
                    .is_ok(),
                None => false,
            };
            return Err(TransferUploadPreparationError {
                transfer_id: transfer.transfer_id.clone(),
                failure_code,
                failure_reported,
                message: error.to_string(),
            });
        }
    };

    Ok(PreparedTransferUpload {
        source,
        upload_target,
    })
}

pub async fn upload_prepared_transfer(
    agent: &AgentRuntime,
    outbox: &OutboxStore,
    prepared: PreparedTransferUpload,
) -> Result<CompletedTransferUpload, TransferUploadExecutionError> {
    if let Err(error) = ensure_source_unchanged(&prepared.source) {
        return Err(report_source_changed(agent, &prepared.source.transfer_id, error).await);
    }

    let payload = match hash_source_payload(&prepared.source) {
        Ok(payload) => payload,
        Err(error) => {
            return Err(report_source_changed(agent, &prepared.source.transfer_id, error).await)
        }
    };

    if let Err(error) = ensure_source_unchanged(&prepared.source) {
        return Err(report_source_changed(agent, &prepared.source.transfer_id, error).await);
    }

    put_upload_file(
        &prepared.upload_target.upload_url,
        &prepared.source.resolved_path,
        payload.size_bytes,
    )
    .await
    .map_err(|message| TransferUploadExecutionError {
        transfer_id: prepared.source.transfer_id.clone(),
        failure_code: None,
        failure_reported: false,
        message,
    })?;

    if let Err(error) = ensure_source_unchanged(&prepared.source) {
        return Err(report_source_changed(agent, &prepared.source.transfer_id, error).await);
    }

    enqueue_file_transfer_completion(
        outbox,
        &prepared.source.transfer_id,
        payload.size_bytes,
        &payload.sha256,
    )
    .map_err(|message| TransferUploadExecutionError {
        transfer_id: prepared.source.transfer_id.clone(),
        failure_code: None,
        failure_reported: false,
        message,
    })?;

    agent
        .complete_file_transfer_upload(
            prepared.source.transfer_id.clone(),
            file_transfer_completion_key(&prepared.source.transfer_id),
            payload.size_bytes,
            payload.sha256.clone(),
        )
        .await
        .map_err(|error| TransferUploadExecutionError {
            transfer_id: prepared.source.transfer_id.clone(),
            failure_code: None,
            failure_reported: false,
            message: error.to_string(),
        })?;

    Ok(CompletedTransferUpload {
        transfer_id: prepared.source.transfer_id,
        size_bytes: payload.size_bytes,
        sha256: payload.sha256,
    })
}

async fn report_source_changed(
    agent: &AgentRuntime,
    transfer_id: &str,
    error: TransferUploadExecutionError,
) -> TransferUploadExecutionError {
    let failure_reported = agent
        .fail_file_transfer(
            transfer_id.to_string(),
            AgentFileTransferFailureCode::SourceChanged,
        )
        .await
        .is_ok();
    TransferUploadExecutionError {
        transfer_id: transfer_id.to_string(),
        failure_code: Some(AgentFileTransferFailureCode::SourceChanged),
        failure_reported,
        message: error.message,
    }
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
        .ensure_active_room_binding(&room.root_id, &transfer.room_id)
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

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TransferUploadPayload {
    pub(crate) size_bytes: u64,
    pub(crate) sha256: String,
}

pub(crate) fn hash_source_payload(
    source: &ValidatedTransferSource,
) -> Result<TransferUploadPayload, TransferUploadExecutionError> {
    let mut file =
        File::open(&source.resolved_path).map_err(|error| TransferUploadExecutionError {
            transfer_id: source.transfer_id.clone(),
            failure_code: Some(AgentFileTransferFailureCode::SourceChanged),
            failure_reported: false,
            message: format!("cannot open source file for upload: {error}"),
        })?;
    let mut hasher = Sha256::new();
    let mut size_bytes = 0_u64;
    let mut buffer = vec![0_u8; FILE_READ_CHUNK_BYTES];

    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| TransferUploadExecutionError {
                transfer_id: source.transfer_id.clone(),
                failure_code: Some(AgentFileTransferFailureCode::SourceChanged),
                failure_reported: false,
                message: format!("cannot read source file for upload: {error}"),
            })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        size_bytes = size_bytes.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
    }

    if size_bytes != source.source_version.size_bytes {
        return Err(TransferUploadExecutionError {
            transfer_id: source.transfer_id.clone(),
            failure_code: Some(AgentFileTransferFailureCode::SourceChanged),
            failure_reported: false,
            message: "source size changed while preparing upload".to_string(),
        });
    }

    Ok(TransferUploadPayload {
        size_bytes,
        sha256: hex_lower(&hasher.finalize()),
    })
}

pub(crate) async fn put_upload_file(
    upload_url: &str,
    path: &PathBuf,
    size_bytes: u64,
) -> Result<(), String> {
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|error| format!("cannot open source file stream for upload: {error}"))?;
    let stream = tokio_util::io::ReaderStream::new(file);
    let body = reqwest::Body::wrap_stream(stream);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(UPLOAD_TIMEOUT_SECONDS))
        .build()
        .map_err(|error| format!("cannot configure upload client: {error}"))?;
    let response = client
        .put(upload_url)
        .header("Content-Length", size_bytes.to_string())
        .body(body)
        .send()
        .await
        .map_err(|error| format!("cannot stream file upload: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "upload target rejected file stream with HTTP {}",
            response.status()
        ));
    }
    Ok(())
}

fn ensure_source_unchanged(
    source: &ValidatedTransferSource,
) -> Result<(), TransferUploadExecutionError> {
    let metadata =
        fs::metadata(&source.resolved_path).map_err(|error| TransferUploadExecutionError {
            transfer_id: source.transfer_id.clone(),
            failure_code: Some(AgentFileTransferFailureCode::SourceChanged),
            failure_reported: false,
            message: format!("cannot re-read source metadata: {error}"),
        })?;
    if !metadata.is_file() {
        return Err(TransferUploadExecutionError {
            transfer_id: source.transfer_id.clone(),
            failure_code: Some(AgentFileTransferFailureCode::SourceChanged),
            failure_reported: false,
            message: "source is no longer a file".to_string(),
        });
    }
    let modified_unix_ms = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis());
    let current = AgentFileTransferSourceVersion {
        file_id: stable_file_id(
            &source.source_relative_path,
            metadata.len(),
            modified_unix_ms,
        ),
        size_bytes: metadata.len(),
        modified_at: unix_ms_to_rfc3339(modified_unix_ms),
    };
    if current != source.source_version {
        return Err(TransferUploadExecutionError {
            transfer_id: source.transfer_id.clone(),
            failure_code: Some(AgentFileTransferFailureCode::SourceChanged),
            failure_reported: false,
            message: "source file changed before upload completion".to_string(),
        });
    }
    Ok(())
}

fn upload_target_failure_code(message: &str) -> Option<AgentFileTransferFailureCode> {
    if message.contains("SIZE_LIMIT_EXCEEDED") {
        Some(AgentFileTransferFailureCode::SizeLimitExceeded)
    } else {
        None
    }
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

fn hex_lower(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
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
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use tempfile::tempdir;

    use super::{
        ensure_source_unchanged, hash_source_payload, prepare_upload_target_for_transfer,
        put_upload_file, unix_ms_to_rfc3339, upload_target_failure_code,
        validate_local_transfer_source,
    };
    use crate::agent::{AgentFileTransfer, AgentFileTransferFailureCode, AgentRuntime};
    use crate::storage::managed_roots::ManagedRootStore;

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

    #[tokio::test]
    async fn skips_non_requested_transfer_before_network_or_filesystem_work() {
        let agent = AgentRuntime::default();
        let roots = ManagedRootStore::default();
        let transfer = AgentFileTransfer {
            transfer_id: "transfer-1".to_string(),
            room_id: "room-1".to_string(),
            source_relative_path: "docs/report.pdf".to_string(),
            status: "UPLOADING".to_string(),
            expires_at: "2026-07-13T00:00:00.000Z".to_string(),
            size_bytes: None,
            sha256: None,
            failure_code: None,
        };

        let error = prepare_upload_target_for_transfer(&agent, &roots, &transfer)
            .await
            .expect_err("non-requested transfer is skipped");

        assert!(error.failure_code.is_none());
        assert!(!error.failure_reported);
        assert!(error.message.contains("not pending"));
    }

    #[test]
    fn maps_size_limit_upload_target_rejection_to_transfer_failure_code() {
        assert_eq!(
            upload_target_failure_code("TRANSPORT_UNAVAILABLE: SIZE_LIMIT_EXCEEDED"),
            Some(AgentFileTransferFailureCode::SizeLimitExceeded)
        );
        assert_eq!(upload_target_failure_code("UNCONFIGURED"), None);
    }

    #[test]
    fn hashes_source_payload_without_buffering_body() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("root");
        fs::write(root.join("hello.txt"), "hello").expect("file");
        let source =
            validate_local_transfer_source(root.to_str().expect("root"), "hello.txt", None, None)
                .expect("source");

        let payload = hash_source_payload(&source).expect("payload");

        assert_eq!(payload.size_bytes, 5);
        assert_eq!(
            payload.sha256,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn source_change_is_detected_before_upload_completion() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("root");
        fs::write(root.join("report.txt"), "old").expect("file");
        let source =
            validate_local_transfer_source(root.to_str().expect("root"), "report.txt", None, None)
                .expect("source");

        fs::write(root.join("report.txt"), "new content").expect("change file");
        let error = ensure_source_unchanged(&source).expect_err("source changed");

        assert_eq!(
            error.failure_code,
            Some(AgentFileTransferFailureCode::SourceChanged)
        );
    }

    #[tokio::test]
    async fn put_upload_file_streams_the_file_body_to_upload_target() {
        let temp = tempdir().expect("tempdir");
        let upload_file = temp.path().join("upload.txt");
        fs::write(&upload_file, "hello upload").expect("upload file");

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind upload target");
        let address = listener.local_addr().expect("address");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept upload");
            let mut request_bytes = Vec::new();
            let mut buffer = [0_u8; 1024];
            loop {
                let length = stream.read(&mut buffer).expect("read upload");
                if length == 0 {
                    break;
                }
                request_bytes.extend_from_slice(&buffer[..length]);
                if request_bytes
                    .windows(b"\r\n\r\nhello upload".len())
                    .any(|window| window == b"\r\n\r\nhello upload")
                {
                    break;
                }
            }
            let request = String::from_utf8_lossy(&request_bytes);
            assert!(request.starts_with("PUT /upload-target HTTP/1.1"));
            assert!(request.ends_with("hello upload"));
            let response = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        put_upload_file(&format!("http://{address}/upload-target"), &upload_file, 12)
            .await
            .expect("put upload");
        server.join().expect("server thread");
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
