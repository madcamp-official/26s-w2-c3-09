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

use crate::agent::AgentRuntime;
use crate::command_processor::AgentProposalSubmission;
use crate::storage::outbox::OutboxStore;

const OUTBOX_FLUSH_BATCH: i64 = 50;

const KIND_EXECUTION_RESULT: &str = "execution_result";
const KIND_PROPOSAL: &str = "proposal";
const KIND_COMMAND_STATUS: &str = "command_status";
const KIND_FILE_TRANSFER_COMPLETION: &str = "file_transfer_completion";

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

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tempfile::tempdir;

    use super::{
        enqueue_command_status, enqueue_execution_result, enqueue_file_transfer_completion,
        enqueue_proposal, file_transfer_completion_key, KIND_COMMAND_STATUS, KIND_EXECUTION_RESULT,
        KIND_FILE_TRANSFER_COMPLETION, KIND_PROPOSAL,
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
}
