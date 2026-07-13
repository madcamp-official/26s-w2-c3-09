use std::collections::BTreeMap;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::agent::{AgentRuntime, AgentSmartCacheCandidate, AgentSmartCacheReservation};
use crate::file_transfer_processor::{
    hash_source_payload, put_upload_file, validate_local_transfer_source, ValidatedTransferSource,
};
use crate::outbox_processor::{enqueue_smart_cache_completion, smart_cache_completion_key};
use crate::storage::managed_roots::ManagedRootStore;
use crate::storage::outbox::OutboxStore;
use crate::storage::smart_cache::{SmartCacheCandidate, SmartCacheStore};

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct SmartCacheProcessingReport {
    pub inspected_count: usize,
    pub submitted_count: usize,
    pub approved_count: usize,
    pub uploaded_count: usize,
    pub failed_count: usize,
    pub skipped_count: usize,
    pub message: Option<String>,
}

#[derive(Clone, Debug)]
struct PreparedCacheCandidate {
    local: SmartCacheCandidate,
    source: ValidatedTransferSource,
    source_version_hash: String,
}

pub async fn process_smart_cache_for_room(
    agent: &AgentRuntime,
    roots: &ManagedRootStore,
    smart_cache: &SmartCacheStore,
    outbox: &OutboxStore,
    room_id: String,
    limit: i64,
) -> Result<SmartCacheProcessingReport, String> {
    let room = agent
        .root_id_for_room(room_id.clone())
        .await
        .map_err(|error| error.to_string())?;
    process_smart_cache_for_known_room(agent, roots, smart_cache, outbox, room, limit).await
}

pub async fn process_smart_cache_for_enabled_rooms(
    agent: &AgentRuntime,
    roots: &ManagedRootStore,
    smart_cache: &SmartCacheStore,
    outbox: &OutboxStore,
    limit_per_room: i64,
) -> Result<SmartCacheProcessingReport, String> {
    let rooms = agent
        .list_rooms()
        .await
        .map_err(|error| error.to_string())?;
    let mut report = SmartCacheProcessingReport {
        inspected_count: 0,
        submitted_count: 0,
        approved_count: 0,
        uploaded_count: 0,
        failed_count: 0,
        skipped_count: 0,
        message: None,
    };

    for room in rooms {
        match process_smart_cache_for_known_room(
            agent,
            roots,
            smart_cache,
            outbox,
            room,
            limit_per_room,
        )
        .await
        {
            Ok(room_report) => merge_report(&mut report, room_report),
            Err(_) => report.failed_count += 1,
        }
    }

    Ok(report)
}

async fn process_smart_cache_for_known_room(
    agent: &AgentRuntime,
    roots: &ManagedRootStore,
    smart_cache: &SmartCacheStore,
    outbox: &OutboxStore,
    room: crate::agent::AgentRoomSync,
    limit: i64,
) -> Result<SmartCacheProcessingReport, String> {
    let policy = agent
        .smart_cache_policy(room.room_id.clone())
        .await
        .map_err(|error| error.to_string())?;
    if !policy.enabled {
        return Ok(SmartCacheProcessingReport {
            inspected_count: 0,
            submitted_count: 0,
            approved_count: 0,
            uploaded_count: 0,
            failed_count: 0,
            skipped_count: 1,
            message: Some("smart cache policy is disabled for this room".to_string()),
        });
    }

    let root = roots.ensure_active_room_binding(&room.root_id, &room.room_id)?;
    if !root.enabled {
        return Ok(SmartCacheProcessingReport {
            inspected_count: 0,
            submitted_count: 0,
            approved_count: 0,
            uploaded_count: 0,
            failed_count: 0,
            skipped_count: 1,
            message: Some(format!("managed root is disabled: {}", root.root_id)),
        });
    }

    let local_candidates = smart_cache.list_candidates(room.root_id.clone(), limit)?;
    let inspected_count = local_candidates.len();
    let mut prepared = Vec::new();
    let mut failed_count = 0;

    for candidate in local_candidates {
        match prepare_candidate(&root.root, candidate, policy.max_file_bytes) {
            Ok(candidate) => prepared.push(candidate),
            Err(_) => failed_count += 1,
        }
    }

    if prepared.is_empty() {
        return Ok(SmartCacheProcessingReport {
            inspected_count,
            submitted_count: 0,
            approved_count: 0,
            uploaded_count: 0,
            failed_count,
            skipped_count: 0,
            message: Some("no smart cache candidates survived local validation".to_string()),
        });
    }

    let server_candidates = prepared
        .iter()
        .map(server_candidate)
        .collect::<Result<Vec<_>, String>>()?;
    let key = candidate_batch_idempotency_key(&room.room_id, &server_candidates)?;
    let submitted_count = server_candidates.len();
    let batch = agent
        .submit_smart_cache_candidates(key, room.room_id.clone(), server_candidates)
        .await
        .map_err(|error| error.to_string())?;

    let by_path_and_version = prepared
        .into_iter()
        .map(|candidate| {
            (
                (
                    candidate.local.relative_path.clone(),
                    candidate.source_version_hash.clone(),
                ),
                candidate,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let approved_count = batch.approved.len();
    let mut uploaded_count = 0;

    for reservation in batch.approved {
        if reservation.status == "COMPLETED" {
            continue;
        }
        match upload_reservation(agent, outbox, &root.root, &by_path_and_version, reservation).await
        {
            Ok(()) => uploaded_count += 1,
            Err(_) => failed_count += 1,
        }
    }

    Ok(SmartCacheProcessingReport {
        inspected_count,
        submitted_count,
        approved_count,
        uploaded_count,
        failed_count,
        skipped_count: usize::try_from(batch.rejected_count).unwrap_or(usize::MAX),
        message: None,
    })
}

fn merge_report(total: &mut SmartCacheProcessingReport, next: SmartCacheProcessingReport) {
    total.inspected_count += next.inspected_count;
    total.submitted_count += next.submitted_count;
    total.approved_count += next.approved_count;
    total.uploaded_count += next.uploaded_count;
    total.failed_count += next.failed_count;
    total.skipped_count += next.skipped_count;
}

fn prepare_candidate(
    root: &str,
    candidate: SmartCacheCandidate,
    max_file_bytes: u64,
) -> Result<PreparedCacheCandidate, String> {
    let source = validate_local_transfer_source(
        root,
        &candidate.relative_path,
        Some(&candidate.relative_path),
        Some(&candidate.root_id),
    )
    .map_err(|error| error.message)?;
    if source.source_version.size_bytes > max_file_bytes {
        return Err("candidate exceeds smart cache maxFileBytes".to_string());
    }
    let source_version_hash = source_version_hash(&source)?;
    Ok(PreparedCacheCandidate {
        local: candidate,
        source,
        source_version_hash,
    })
}

fn server_candidate(
    candidate: &PreparedCacheCandidate,
) -> Result<AgentSmartCacheCandidate, String> {
    Ok(AgentSmartCacheCandidate {
        source_relative_path: candidate.source.source_relative_path.clone(),
        source_version: serde_json::to_value(&candidate.source.source_version)
            .map_err(|error| format!("cannot encode source version: {error}"))?,
        source_version_hash: candidate.source_version_hash.clone(),
        size_bytes: candidate.source.source_version.size_bytes,
        usage_score: candidate.local.score,
        manual_pin: candidate.local.pinned,
    })
}

async fn upload_reservation(
    agent: &AgentRuntime,
    outbox: &OutboxStore,
    root: &str,
    candidates: &BTreeMap<(String, String), PreparedCacheCandidate>,
    reservation: AgentSmartCacheReservation,
) -> Result<(), String> {
    let reservation_id = reservation.reservation_id.clone();
    let prepared = async {
        let Some(upload_url) = reservation.upload_url.clone() else {
            return Err("smart cache reservation omitted upload URL".to_string());
        };
        let key = (
            reservation.source_relative_path.clone(),
            reservation.source_version_hash.clone(),
        );
        let candidate = candidates
            .get(&key)
            .ok_or_else(|| "smart cache reservation did not match a local candidate".to_string())?;
        ensure_candidate_still_current(root, candidate)?;
        let payload = hash_source_payload(&candidate.source).map_err(|error| error.message)?;
        if payload.size_bytes != reservation.size_bytes {
            return Err("smart cache reservation size no longer matches local source".to_string());
        }
        put_upload_file(
            &upload_url,
            &candidate.source.resolved_path,
            payload.size_bytes,
        )
        .await?;
        ensure_candidate_still_current(root, candidate)?;
        Ok::<_, String>((payload, candidate.local.score, candidate.local.pinned))
    }
    .await;
    let (payload, usage_score, manual_pin) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            let _ = agent.cancel_smart_cache_reservation(reservation_id).await;
            return Err(error);
        }
    };

    if let Err(error) = enqueue_smart_cache_completion(
        outbox,
        &reservation.reservation_id,
        payload.size_bytes,
        &payload.sha256,
        usage_score,
        manual_pin,
    ) {
        let _ = agent.cancel_smart_cache_reservation(reservation_id).await;
        return Err(error);
    }
    agent
        .complete_smart_cache_upload(
            reservation.reservation_id.clone(),
            smart_cache_completion_key(&reservation.reservation_id),
            payload.size_bytes,
            payload.sha256,
            usage_score,
            manual_pin,
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn ensure_candidate_still_current(
    root: &str,
    candidate: &PreparedCacheCandidate,
) -> Result<(), String> {
    let current = validate_local_transfer_source(
        root,
        &candidate.local.relative_path,
        Some(&candidate.local.relative_path),
        Some(&candidate.local.root_id),
    )
    .map_err(|error| error.message)?;
    let current_hash = source_version_hash_for_version(&current.source_version)?;
    if current.source_version != candidate.source.source_version
        || current_hash != candidate.source_version_hash
    {
        return Err("smart cache source changed before upload completion".to_string());
    }
    Ok(())
}

fn source_version_hash(source: &ValidatedTransferSource) -> Result<String, String> {
    source_version_hash_for_version(&source.source_version)
}

fn source_version_hash_for_version(
    version: &crate::agent::AgentFileTransferSourceVersion,
) -> Result<String, String> {
    let bytes = serde_json::to_vec(version)
        .map_err(|error| format!("cannot encode source version: {error}"))?;
    Ok(sha256_hex(&bytes))
}

fn candidate_batch_idempotency_key(
    room_id: &str,
    candidates: &[AgentSmartCacheCandidate],
) -> Result<String, String> {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "roomId": room_id,
        "candidates": candidates,
    }))
    .map_err(|error| format!("cannot encode smart cache candidate batch: {error}"))?;
    Ok(format!("cache-{}", &sha256_hex(&bytes)[..32]))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::candidate_batch_idempotency_key;
    use crate::agent::AgentSmartCacheCandidate;
    use serde_json::json;

    #[test]
    fn candidate_batch_key_is_stable_for_same_body() {
        let candidates = vec![AgentSmartCacheCandidate {
            source_relative_path: "docs/report.pdf".to_string(),
            source_version: json!({"fileId":"hm:1","sizeBytes":10,"modifiedAt":"2026-07-13T00:00:00.000Z"}),
            source_version_hash: "a".repeat(64),
            size_bytes: 10,
            usage_score: 42,
            manual_pin: false,
        }];

        assert_eq!(
            candidate_batch_idempotency_key("room-1", &candidates).expect("key"),
            candidate_batch_idempotency_key("room-1", &candidates).expect("key again")
        );
    }
}
