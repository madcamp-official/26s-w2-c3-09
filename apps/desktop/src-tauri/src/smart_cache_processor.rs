use std::collections::BTreeMap;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::agent::{AgentRuntime, AgentSmartCacheCandidate, AgentSmartCacheReservation};
use crate::file_transfer_processor::{
    put_upload_bytes, validate_local_transfer_source, ValidatedTransferSource,
};
use crate::outbox_processor::{enqueue_smart_cache_completion, smart_cache_completion_key};
use crate::smart_cache_crypto::{encrypt_smart_cache_file, encrypted_smart_cache_object_size};
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
    let (local_candidates, policy_skipped_count) =
        filter_policy_excluded_candidates(local_candidates, &policy.excluded_patterns);
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
            skipped_count: policy_skipped_count,
            message: Some(
                "no smart cache candidates survived local policy or validation".to_string(),
            ),
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
        skipped_count: policy_skipped_count
            .saturating_add(usize::try_from(batch.rejected_count).unwrap_or(usize::MAX)),
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

fn filter_policy_excluded_candidates(
    candidates: Vec<SmartCacheCandidate>,
    excluded_patterns: &[String],
) -> (Vec<SmartCacheCandidate>, usize) {
    if excluded_patterns.is_empty() {
        return (candidates, 0);
    }

    let inspected_count = candidates.len();
    let filtered = candidates
        .into_iter()
        .filter(|candidate| {
            !matches_policy_excluded_pattern(&candidate.relative_path, excluded_patterns)
        })
        .collect::<Vec<_>>();
    let skipped_count = inspected_count.saturating_sub(filtered.len());
    (filtered, skipped_count)
}

fn matches_policy_excluded_pattern(relative_path: &str, excluded_patterns: &[String]) -> bool {
    let normalized_path = relative_path.replace('\\', "/");
    excluded_patterns
        .iter()
        .any(|pattern| glob_matches(&normalize_policy_pattern(pattern), &normalized_path))
}

fn normalize_policy_pattern(pattern: &str) -> String {
    pattern.trim().replace('\\', "/")
}

fn glob_matches(pattern: &str, value: &str) -> bool {
    let tokens = parse_policy_glob(pattern);
    let value = value.chars().collect::<Vec<_>>();
    let mut reachable = vec![vec![false; value.len() + 1]; tokens.len() + 1];
    reachable[0][0] = true;

    for token_index in 0..tokens.len() {
        for value_index in 0..=value.len() {
            if !reachable[token_index][value_index] {
                continue;
            }
            match tokens[token_index] {
                PolicyGlobToken::Literal(expected) => {
                    if value_index < value.len() && value[value_index] == expected {
                        reachable[token_index + 1][value_index + 1] = true;
                    }
                }
                PolicyGlobToken::AnySegment => {
                    reachable[token_index + 1][value_index] = true;
                    let mut next_index = value_index;
                    while next_index < value.len() && value[next_index] != '/' {
                        next_index += 1;
                        reachable[token_index + 1][next_index] = true;
                    }
                }
                PolicyGlobToken::OneSegment => {
                    if value_index < value.len() && value[value_index] != '/' {
                        reachable[token_index + 1][value_index + 1] = true;
                    }
                }
                PolicyGlobToken::AnyDeep => {
                    reachable[token_index + 1][value_index] = true;
                    for next_index in value_index..value.len() {
                        reachable[token_index + 1][next_index + 1] = true;
                    }
                }
            }
        }
    }

    reachable[tokens.len()][value.len()]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PolicyGlobToken {
    Literal(char),
    AnySegment,
    OneSegment,
    AnyDeep,
}

fn parse_policy_glob(pattern: &str) -> Vec<PolicyGlobToken> {
    let mut tokens = Vec::new();
    let mut chars = pattern.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '*' && chars.peek() == Some(&'*') {
            let _ = chars.next();
            tokens.push(PolicyGlobToken::AnyDeep);
        } else if character == '*' {
            tokens.push(PolicyGlobToken::AnySegment);
        } else if character == '?' {
            tokens.push(PolicyGlobToken::OneSegment);
        } else {
            tokens.push(PolicyGlobToken::Literal(character));
        }
    }
    tokens
}

fn server_candidate(
    candidate: &PreparedCacheCandidate,
) -> Result<AgentSmartCacheCandidate, String> {
    Ok(AgentSmartCacheCandidate {
        source_relative_path: candidate.source.source_relative_path.clone(),
        source_version: serde_json::to_value(&candidate.source.source_version)
            .map_err(|error| format!("cannot encode source version: {error}"))?,
        source_version_hash: candidate.source_version_hash.clone(),
        size_bytes: encrypted_smart_cache_object_size(candidate.source.source_version.size_bytes)?,
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
        let mut encrypted = encrypt_smart_cache_file(
            &candidate.source.resolved_path,
            candidate.source.source_version.size_bytes,
        )?;
        if encrypted.size_bytes != reservation.size_bytes {
            return Err("smart cache reservation size no longer matches local source".to_string());
        }
        put_upload_bytes(&upload_url, std::mem::take(&mut encrypted.bytes)).await?;
        ensure_candidate_still_current(root, candidate)?;
        Ok::<_, String>((encrypted, candidate.local.score, candidate.local.pinned))
    }
    .await;
    let (encrypted, usage_score, manual_pin) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            let _ = agent.cancel_smart_cache_reservation(reservation_id).await;
            return Err(error);
        }
    };

    if let Err(error) = enqueue_smart_cache_completion(
        outbox,
        &reservation.reservation_id,
        encrypted.size_bytes,
        &encrypted.sha256,
        usage_score,
        manual_pin,
        encrypted.metadata.clone(),
    ) {
        let _ = agent.cancel_smart_cache_reservation(reservation_id).await;
        return Err(error);
    }
    agent
        .complete_smart_cache_upload(
            reservation.reservation_id.clone(),
            smart_cache_completion_key(&reservation.reservation_id),
            encrypted.size_bytes,
            encrypted.sha256,
            usage_score,
            manual_pin,
            encrypted.metadata,
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
    use super::{
        candidate_batch_idempotency_key, filter_policy_excluded_candidates,
        matches_policy_excluded_pattern,
    };
    use crate::agent::AgentSmartCacheCandidate;
    use crate::storage::smart_cache::SmartCacheCandidate;
    use serde_json::json;

    #[test]
    fn candidate_batch_key_is_stable_for_same_body() {
        let candidates = vec![AgentSmartCacheCandidate {
            source_relative_path: "docs/report.pdf".to_string(),
            source_version: json!({"fileId":"hm:1","sizeBytes":10,"modifiedAt":"2026-07-13T00:00:00.000Z"}),
            source_version_hash: "a".repeat(64),
            size_bytes: 42,
            usage_score: 42,
            manual_pin: false,
        }];

        assert_eq!(
            candidate_batch_idempotency_key("room-1", &candidates).expect("key"),
            candidate_batch_idempotency_key("room-1", &candidates).expect("key again")
        );
    }

    #[test]
    fn policy_exclusion_globs_match_server_contract() {
        let patterns = vec![
            "private/**".to_string(),
            "*.tmp".to_string(),
            "literal[1].txt".to_string(),
            "notes/??.md".to_string(),
        ];

        assert!(matches_policy_excluded_pattern(
            "private/report.pdf",
            &patterns
        ));
        assert!(matches_policy_excluded_pattern("notes.tmp", &patterns));
        assert!(matches_policy_excluded_pattern("literal[1].txt", &patterns));
        assert!(matches_policy_excluded_pattern("notes/ab.md", &patterns));
        assert!(!matches_policy_excluded_pattern(
            "nested/notes.tmp",
            &patterns
        ));
        assert!(!matches_policy_excluded_pattern("literal1.txt", &patterns));
        assert!(!matches_policy_excluded_pattern("notes/abc.md", &patterns));
    }

    #[test]
    fn policy_exclusion_filter_counts_skipped_candidates() {
        let candidates = vec![
            candidate("private/report.pdf", 30),
            candidate("docs/keep.pdf", 20),
            candidate("notes.tmp", 10),
        ];

        let (allowed, skipped_count) = filter_policy_excluded_candidates(
            candidates,
            &["private/**".to_string(), "*.tmp".to_string()],
        );

        assert_eq!(skipped_count, 2);
        assert_eq!(allowed.len(), 1);
        assert_eq!(allowed[0].relative_path, "docs/keep.pdf");
    }

    fn candidate(relative_path: &str, score: i64) -> SmartCacheCandidate {
        SmartCacheCandidate {
            root_id: "root-1".to_string(),
            relative_path: relative_path.to_string(),
            score,
            event_count: 1,
            last_used_unix_ms: 100,
            pinned: false,
        }
    }
}
