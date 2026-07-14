//! Durable delivery of desktop→server mutations.
//!
//! Producers ([`enqueue_execution_result`], [`enqueue_proposal`], [`enqueue_command_status`]) write
//! a mutation to the local SQLite outbox instead of calling the server inline. [`flush_outbox`],
//! driven by the background loop, then delivers pending rows and applies retry/terminal policy.
//! This is what keeps an execution result from being lost when the network fails *after* the local
//! files have already been moved: the result is durably queued before any network call, and every
//! stored mutation carries the same idempotency key it will be sent with, so redelivery is safe.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::agent::AgentRuntime;
use crate::cleanliness::CleanlinessSnapshot;
use crate::command_processor::AgentProposalSubmission;
use crate::smart_cache_crypto::SmartCacheEncryptionMetadata;
use crate::storage::outbox::OutboxStore;

const OUTBOX_FLUSH_BATCH: i64 = 50;

const KIND_EXECUTION_RESULT: &str = "execution_result";
const KIND_PROPOSAL: &str = "proposal";
const KIND_COMMAND_STATUS: &str = "command_status";
const KIND_FILE_TRANSFER_COMPLETION: &str = "file_transfer_completion";
const KIND_SMART_CACHE_COMPLETION: &str = "smart_cache_completion";
const KIND_SMART_CACHE_STALE_PREFIX: &str = "smart_cache_stale:";
const KIND_ROOM_SNAPSHOT_PREFIX: &str = "room_snapshot:";

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct OutboxFlushReport {
    pub inspected_count: usize,
    pub sent_count: usize,
    pub retried_count: usize,
    pub failed_count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ExecutionResultPayload {
    execution_id: String,
    status: String,
    result_summary: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CommandStatusPayload {
    command_id: String,
    status: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct FileTransferCompletionPayload {
    transfer_id: String,
    size_bytes: u64,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct SmartCacheCompletionPayload {
    reservation_id: String,
    size_bytes: u64,
    sha256: String,
    usage_score: i64,
    manual_pin: bool,
    encryption_metadata: SmartCacheEncryptionMetadata,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct SmartCacheStalePayload {
    room_id: String,
    source_relative_path: Option<String>,
    reason: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct RoomSnapshotPayload {
    room_id: String,
    snapshot: CleanlinessSnapshot,
}

/// Queues the terminal result of a claimed execution. Uses the execution id as both the dedup key
/// and the server idempotency key.
pub fn enqueue_execution_result(
    outbox: &OutboxStore,
    execution_id: &str,
    status: &str,
    result_summary: Value,
) -> Result<(), String> {
    let payload = ExecutionResultPayload {
        execution_id: execution_id.to_string(),
        status: status.to_string(),
        result_summary,
    };
    let payload_json = serde_json::to_string(&payload)
        .map_err(|error| format!("cannot encode execution result: {error}"))?;
    outbox.enqueue(KIND_EXECUTION_RESULT, execution_id, &payload_json)
}

/// Queues a proposal submission. The command id is the dedup key and the server idempotency key.
pub fn enqueue_proposal(
    outbox: &OutboxStore,
    submission: &AgentProposalSubmission,
) -> Result<(), String> {
    let payload_json = serde_json::to_string(submission)
        .map_err(|error| format!("cannot encode proposal submission: {error}"))?;
    outbox.enqueue(KIND_PROPOSAL, &submission.command_id, &payload_json)
}

/// Queues a command status update (used for the terminal FAILED report).
pub fn enqueue_command_status(
    outbox: &OutboxStore,
    command_id: &str,
    status: &str,
) -> Result<(), String> {
    let payload = CommandStatusPayload {
        command_id: command_id.to_string(),
        status: status.to_string(),
    };
    let payload_json = serde_json::to_string(&payload)
        .map_err(|error| format!("cannot encode command status: {error}"))?;
    // Key by command + status so an ANALYZING and a later FAILED for the same command stay distinct.
    let key = format!("{command_id}:{status}");
    outbox.enqueue(KIND_COMMAND_STATUS, &key, &payload_json)
}

pub fn enqueue_file_transfer_completion(
    outbox: &OutboxStore,
    transfer_id: &str,
    size_bytes: u64,
    sha256: &str,
) -> Result<(), String> {
    let payload = FileTransferCompletionPayload {
        transfer_id: transfer_id.to_string(),
        size_bytes,
        sha256: sha256.to_string(),
    };
    let payload_json = serde_json::to_string(&payload)
        .map_err(|error| format!("cannot encode file transfer completion: {error}"))?;
    let key = file_transfer_completion_key(transfer_id);
    outbox.enqueue(KIND_FILE_TRANSFER_COMPLETION, &key, &payload_json)
}

pub fn enqueue_smart_cache_completion(
    outbox: &OutboxStore,
    reservation_id: &str,
    size_bytes: u64,
    sha256: &str,
    usage_score: i64,
    manual_pin: bool,
    encryption_metadata: SmartCacheEncryptionMetadata,
) -> Result<(), String> {
    let payload = SmartCacheCompletionPayload {
        reservation_id: reservation_id.to_string(),
        size_bytes,
        sha256: sha256.to_string(),
        usage_score,
        manual_pin,
        encryption_metadata,
    };
    let payload_json = serde_json::to_string(&payload)
        .map_err(|error| format!("cannot encode smart cache completion: {error}"))?;
    let key = smart_cache_completion_key(reservation_id);
    outbox.enqueue(KIND_SMART_CACHE_COMPLETION, &key, &payload_json)
}

pub fn enqueue_smart_cache_stale(
    outbox: &OutboxStore,
    room_id: &str,
    source_relative_path: Option<&str>,
    reason: &str,
) -> Result<bool, String> {
    if room_id.trim().is_empty() {
        return Err("smart cache stale room id is required".to_string());
    }
    match (source_relative_path, reason) {
        (Some(path), "SOURCE_CHANGED" | "SOURCE_REMOVED") => {
            validate_stale_source_path(path)?;
        }
        (None, "REINDEXED") => {}
        (None, "SOURCE_CHANGED" | "SOURCE_REMOVED") => {
            return Err("smart cache stale source path is required".to_string());
        }
        (Some(_), "REINDEXED") => {
            return Err("smart cache REINDEXED reports must not include a source path".to_string());
        }
        _ => return Err("smart cache stale reason is invalid".to_string()),
    }
    let payload = SmartCacheStalePayload {
        room_id: room_id.to_string(),
        source_relative_path: source_relative_path.map(ToString::to_string),
        reason: reason.to_string(),
    };
    let payload_json = serde_json::to_string(&payload)
        .map_err(|error| format!("cannot encode smart cache stale report: {error}"))?;
    let path_scope = source_relative_path.unwrap_or("*");
    let mut scope = Sha256::new();
    scope.update(room_id.as_bytes());
    scope.update([0]);
    scope.update(path_scope.as_bytes());
    let kind = format!("{KIND_SMART_CACHE_STALE_PREFIX}{:x}", scope.finalize());
    let key = format!("{:039}-{reason}", unix_nanos());
    outbox.enqueue_replacing_pending(&kind, &key, &payload_json)
}

/// Persists the exact snapshot object calculated by the Rust engine. A newer snapshot supersedes
/// older unsent values for this room so a delayed retry cannot move the room sequence backwards.
pub fn enqueue_room_snapshot(
    outbox: &OutboxStore,
    room_id: &str,
    snapshot: &CleanlinessSnapshot,
) -> Result<bool, String> {
    let payload = RoomSnapshotPayload {
        room_id: room_id.to_string(),
        snapshot: snapshot.clone(),
    };
    let payload_json = serde_json::to_string(&payload)
        .map_err(|error| format!("cannot encode room snapshot: {error}"))?;
    let kind = format!("{KIND_ROOM_SNAPSHOT_PREFIX}{room_id}");
    let key = format!("{room_id}-{}", snapshot.calculated_at);
    outbox.enqueue_replacing_pending(&kind, &key, &payload_json)
}

/// Delivers pending outbox rows to the server. Transient failures leave the row pending for the
/// next flush; terminal failures (bad payload, server rejection) dead-letter the row so it cannot
/// retry forever. Only leak-free error codes are stored.
pub async fn flush_outbox(
    agent: &AgentRuntime,
    outbox: &OutboxStore,
) -> Result<OutboxFlushReport, String> {
    let items = outbox.pending_batch(OUTBOX_FLUSH_BATCH)?;
    let mut report = OutboxFlushReport {
        inspected_count: items.len(),
        sent_count: 0,
        retried_count: 0,
        failed_count: 0,
    };

    for item in items {
        match dispatch(agent, &item).await {
            Ok(()) => {
                outbox.mark_sent(item.id)?;
                report.sent_count += 1;
            }
            Err(DispatchError::Transient(code)) => {
                outbox.mark_retry(item.id, &code)?;
                report.retried_count += 1;
            }
            Err(DispatchError::Terminal(code)) => {
                outbox.mark_failed(item.id, &code)?;
                report.failed_count += 1;
            }
        }
    }

    Ok(report)
}

enum DispatchError {
    Transient(String),
    Terminal(String),
}

async fn dispatch(
    agent: &AgentRuntime,
    item: &crate::storage::outbox::OutboxItem,
) -> Result<(), DispatchError> {
    if item.kind.starts_with(KIND_ROOM_SNAPSHOT_PREFIX) {
        let payload = decode::<RoomSnapshotPayload>(&item.payload_json)?;
        if item.kind != format!("{KIND_ROOM_SNAPSHOT_PREFIX}{}", payload.room_id) {
            return Err(DispatchError::Terminal("CORRUPT_PAYLOAD".to_string()));
        }
        return agent
            .submit_room_snapshot(payload.room_id, payload.snapshot)
            .await
            .map(|_| ())
            .map_err(classify);
    }
    if item.kind.starts_with(KIND_SMART_CACHE_STALE_PREFIX) {
        let payload = decode::<SmartCacheStalePayload>(&item.payload_json)?;
        return agent
            .mark_smart_cache_stale(
                item.idempotency_key.clone(),
                payload.room_id,
                payload.source_relative_path,
                payload.reason,
            )
            .await
            .map(|_| ())
            .map_err(classify);
    }
    match item.kind.as_str() {
        KIND_EXECUTION_RESULT => {
            let payload = decode::<ExecutionResultPayload>(&item.payload_json)?;
            agent
                .update_execution(payload.execution_id, payload.status, payload.result_summary)
                .await
                .map(|_| ())
                .map_err(classify)
        }
        KIND_PROPOSAL => {
            let submission = decode::<AgentProposalSubmission>(&item.payload_json)?;
            agent
                .submit_proposal(item.idempotency_key.clone(), submission)
                .await
                .map_err(classify)
        }
        KIND_COMMAND_STATUS => {
            let payload = decode::<CommandStatusPayload>(&item.payload_json)?;
            agent
                .update_command_status(payload.command_id, payload.status)
                .await
                .map(|_| ())
                .map_err(classify)
        }
        KIND_FILE_TRANSFER_COMPLETION => {
            let payload = decode::<FileTransferCompletionPayload>(&item.payload_json)?;
            agent
                .complete_file_transfer_upload(
                    payload.transfer_id,
                    item.idempotency_key.clone(),
                    payload.size_bytes,
                    payload.sha256,
                )
                .await
                .map(|_| ())
                .map_err(classify)
        }
        KIND_SMART_CACHE_COMPLETION => {
            let payload = decode::<SmartCacheCompletionPayload>(&item.payload_json)?;
            agent
                .complete_smart_cache_upload(
                    payload.reservation_id,
                    item.idempotency_key.clone(),
                    payload.size_bytes,
                    payload.sha256,
                    payload.usage_score,
                    payload.manual_pin,
                    payload.encryption_metadata,
                )
                .await
                .map(|_| ())
                .map_err(classify)
        }
        // A row with an unknown kind cannot be delivered by this build; dead-letter it rather than
        // retrying forever.
        other => Err(DispatchError::Terminal(format!("UNKNOWN_KIND:{other}"))),
    }
}

fn decode<T: for<'de> Deserialize<'de>>(payload_json: &str) -> Result<T, DispatchError> {
    serde_json::from_str(payload_json)
        .map_err(|_| DispatchError::Terminal("CORRUPT_PAYLOAD".to_string()))
}

fn classify(error: crate::agent::AgentError) -> DispatchError {
    let code = error.code.as_str().to_string();
    if error.is_transient() {
        DispatchError::Transient(code)
    } else {
        DispatchError::Terminal(code)
    }
}

pub fn file_transfer_completion_key(transfer_id: &str) -> String {
    format!("{transfer_id}-complete")
}

pub fn smart_cache_completion_key(reservation_id: &str) -> String {
    format!("{reservation_id}-complete")
}

fn unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn validate_stale_source_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.len() > 1024
        || path.contains('\0')
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.contains("//")
        || path.contains("\\\\")
    {
        return Err("smart cache stale source path is invalid".to_string());
    }
    for segment in path.split(['/', '\\']) {
        if segment.is_empty() || matches!(segment, "." | "..") {
            return Err("smart cache stale source path is invalid".to_string());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tempfile::tempdir;

    use super::{
        enqueue_command_status, enqueue_execution_result, enqueue_file_transfer_completion,
        enqueue_proposal, enqueue_room_snapshot, enqueue_smart_cache_completion,
        enqueue_smart_cache_stale, file_transfer_completion_key, smart_cache_completion_key,
        RoomSnapshotPayload, SmartCacheStalePayload, KIND_COMMAND_STATUS, KIND_EXECUTION_RESULT,
        KIND_FILE_TRANSFER_COMPLETION, KIND_PROPOSAL, KIND_ROOM_SNAPSHOT_PREFIX,
        KIND_SMART_CACHE_COMPLETION, KIND_SMART_CACHE_STALE_PREFIX,
    };
    use crate::command_processor::{
        AgentProposalActionType, AgentProposalConflictState, AgentProposalItem,
        AgentProposalSubmission, AgentProposalSummary,
    };
    use crate::storage::outbox::OutboxStore;

    fn store() -> (tempfile::TempDir, OutboxStore) {
        let temp = tempdir().expect("temp dir");
        let store = OutboxStore::default();
        store
            .load_from_db(temp.path().join("outbox.db"))
            .expect("load outbox");
        (temp, store)
    }

    fn sample_submission() -> AgentProposalSubmission {
        AgentProposalSubmission {
            command_id: "cmd-1".to_string(),
            room_id: "room-1".to_string(),
            summary: AgentProposalSummary {
                item_count: 1,
                readme_draft: None,
                readme_diff: None,
            },
            expires_at: None,
            items: vec![AgentProposalItem {
                item_order: 0,
                action_type: AgentProposalActionType::Move,
                source_relative_path: Some("a.pdf".to_string()),
                destination_relative_path: Some("Docs/a.pdf".to_string()),
                reason_code: "RULE_MOVE_BY_EXTENSION".to_string(),
                precondition: json!({ "sourceSizeBytes": 3 }),
                conflict_state: AgentProposalConflictState::None,
            }],
        }
    }

    #[test]
    fn execution_result_is_queued_under_its_execution_id() {
        let (_temp, store) = store();
        enqueue_execution_result(&store, "exec-1", "SUCCEEDED", json!({ "executedCount": 1 }))
            .expect("enqueue");

        let batch = store.pending_batch(10).expect("batch");
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].kind, KIND_EXECUTION_RESULT);
        assert_eq!(batch[0].idempotency_key, "exec-1");
    }

    #[test]
    fn proposal_round_trips_through_the_outbox_payload() {
        let (_temp, store) = store();
        let submission = sample_submission();
        enqueue_proposal(&store, &submission).expect("enqueue");

        let batch = store.pending_batch(10).expect("batch");
        assert_eq!(batch[0].kind, KIND_PROPOSAL);
        assert_eq!(batch[0].idempotency_key, "cmd-1");
        // The stored payload must deserialize back into the exact submission the flush loop sends.
        let restored: AgentProposalSubmission =
            serde_json::from_str(&batch[0].payload_json).expect("decode submission");
        assert_eq!(restored, submission);
    }

    #[test]
    fn command_status_key_separates_distinct_statuses() {
        let (_temp, store) = store();
        enqueue_command_status(&store, "cmd-1", "ANALYZING").expect("enqueue analyzing");
        enqueue_command_status(&store, "cmd-1", "FAILED").expect("enqueue failed");

        let batch = store.pending_batch(10).expect("batch");
        assert_eq!(batch.len(), 2);
        assert!(batch.iter().all(|item| item.kind == KIND_COMMAND_STATUS));
    }

    #[test]
    fn file_transfer_completion_is_queued_under_stable_completion_key() {
        let (_temp, store) = store();
        enqueue_file_transfer_completion(&store, "transfer-1", 5, &"a".repeat(64))
            .expect("enqueue completion");

        let batch = store.pending_batch(10).expect("batch");

        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].kind, KIND_FILE_TRANSFER_COMPLETION);
        assert_eq!(
            batch[0].idempotency_key,
            file_transfer_completion_key("transfer-1")
        );
        assert!(batch[0].payload_json.contains(r#""sizeBytes":5"#));
    }

    #[test]
    fn smart_cache_completion_is_queued_under_stable_completion_key() {
        let (_temp, store) = store();
        enqueue_smart_cache_completion(
            &store,
            "reservation-1",
            37,
            &"b".repeat(64),
            42,
            true,
            crate::smart_cache_crypto::SmartCacheEncryptionMetadata {
                algorithm: "AES-256-GCM".to_string(),
                format: "MKS1_NONCE_CIPHERTEXT_TAG".to_string(),
                key_id: "mks1-test-key-1234".to_string(),
                nonce_hex: "c".repeat(24),
                plaintext_size_bytes: 5,
                plaintext_sha256: "d".repeat(64),
            },
        )
        .expect("enqueue completion");

        let batch = store.pending_batch(10).expect("batch");

        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].kind, KIND_SMART_CACHE_COMPLETION);
        assert_eq!(
            batch[0].idempotency_key,
            smart_cache_completion_key("reservation-1")
        );
        assert!(batch[0].payload_json.contains(r#""usageScore":42"#));
        assert!(batch[0].payload_json.contains(r#""manualPin":true"#));
        assert!(batch[0].payload_json.contains(r#""encryptionMetadata":"#));
        assert!(batch[0].payload_json.contains(r#""plaintextSizeBytes":5"#));
    }

    #[test]
    fn smart_cache_stale_reports_replace_pending_reports_for_the_same_source() {
        let (_temp, store) = store();
        assert!(
            enqueue_smart_cache_stale(&store, "room-1", Some("docs/a.pdf"), "SOURCE_CHANGED")
                .expect("first stale report")
        );
        enqueue_smart_cache_stale(&store, "room-1", Some("docs/a.pdf"), "SOURCE_CHANGED")
            .expect("second stale report");

        let batch = store.pending_batch(10).expect("batch");

        assert_eq!(batch.len(), 1);
        assert!(batch[0].kind.starts_with(KIND_SMART_CACHE_STALE_PREFIX));
        let payload: SmartCacheStalePayload =
            serde_json::from_str(&batch[0].payload_json).expect("decode stale payload");
        assert_eq!(payload.room_id, "room-1");
        assert_eq!(payload.source_relative_path.as_deref(), Some("docs/a.pdf"));
        assert_eq!(payload.reason, "SOURCE_CHANGED");
    }

    #[test]
    fn smart_cache_stale_rejects_escape_paths_before_outbox_persistence() {
        let (_temp, store) = store();

        let error =
            enqueue_smart_cache_stale(&store, "room-1", Some("../secret.txt"), "SOURCE_CHANGED")
                .expect_err("escape path must fail");

        assert!(error.contains("source path"));
        assert!(store.pending_batch(10).expect("batch").is_empty());
    }

    #[test]
    fn room_snapshot_outbox_keeps_the_exact_newest_object() {
        let (_temp, store) = store();
        let mut older = crate::cleanliness::CleanlinessSnapshot {
            formula_version: crate::cleanliness::CLEANLINESS_FORMULA_VERSION.to_string(),
            score: 10,
            metrics: crate::cleanliness::CleanlinessMetrics {
                total_file_count: 2,
                managed_file_count: 1,
                unorganized_file_count: 1,
                deductions: vec![crate::cleanliness::CleanlinessDeduction {
                    reason_code: "UNORGANIZED_FILES".to_string(),
                    count: 1,
                    points: 50,
                }],
            },
            calculated_at: "2026-07-13T00:00:00.001Z".to_string(),
        };
        enqueue_room_snapshot(&store, "room-1", &older).expect("older");
        older.score = 50;
        older.calculated_at = "2026-07-13T00:00:00.002Z".to_string();
        let newest = older;
        enqueue_room_snapshot(&store, "room-1", &newest).expect("newest");

        let batch = store.pending_batch(10).expect("batch");
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].kind, format!("{KIND_ROOM_SNAPSHOT_PREFIX}room-1"));
        let decoded: RoomSnapshotPayload =
            serde_json::from_str(&batch[0].payload_json).expect("payload");
        assert_eq!(decoded.room_id, "room-1");
        assert_eq!(decoded.snapshot, newest);
    }

    #[test]
    fn delayed_older_snapshot_cannot_replace_a_newer_sequence() {
        let (_temp, store) = store();
        let newer = crate::cleanliness::CleanlinessSnapshot {
            formula_version: crate::cleanliness::CLEANLINESS_FORMULA_VERSION.to_string(),
            score: 90,
            metrics: crate::cleanliness::CleanlinessMetrics {
                total_file_count: 1,
                managed_file_count: 1,
                unorganized_file_count: 0,
                deductions: Vec::new(),
            },
            calculated_at: "2026-07-13T00:00:00.002Z".to_string(),
        };
        let mut older = newer.clone();
        older.score = 10;
        older.calculated_at = "2026-07-13T00:00:00.001Z".to_string();

        assert!(enqueue_room_snapshot(&store, "room-1", &newer).expect("newer"));
        assert!(!enqueue_room_snapshot(&store, "room-1", &older).expect("delayed older"));

        let batch = store.pending_batch(10).expect("batch");
        assert_eq!(batch.len(), 1);
        let decoded: RoomSnapshotPayload =
            serde_json::from_str(&batch[0].payload_json).expect("payload");
        assert_eq!(decoded.snapshot, newer);

        store.mark_sent(batch[0].id).expect("mark newer sent");
        assert!(!enqueue_room_snapshot(&store, "room-1", &older).expect("older after send"));
        assert!(store
            .pending_batch(10)
            .expect("no regression pending")
            .is_empty());
    }
}
