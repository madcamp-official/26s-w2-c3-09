use std::time::{SystemTime, UNIX_EPOCH};

use file_engine_cli::browse::{browse_root, BrowseEntry, BrowseError};
use file_engine_cli::file_index::{
    index_is_initialized, reindex_root, search_index_page_with_extensions, FileIndexError,
    IndexedSearchEntry, IndexedSearchScope,
};
use serde::Serialize;

use crate::agent::{
    AgentFileBrowseEntry, AgentFileBrowseEntryType, AgentFileBrowseFailureCode,
    AgentFileBrowseRequest, AgentFileBrowseResult, AgentFileSearchScope, AgentRuntime,
};
use crate::storage::managed_roots::ManagedRootStore;

const PAGE_SIZE: usize = 200;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct FileBrowseProcessingReport {
    pub inspected_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    pub results: Vec<FileBrowseProcessingResult>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct FileBrowseProcessingResult {
    pub request_id: String,
    pub status: FileBrowseProcessingStatus,
    pub entry_count: usize,
    pub next_cursor: Option<String>,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileBrowseProcessingStatus {
    Completed,
    Failed,
}

pub async fn process_pending_file_browse_requests(
    agent: &AgentRuntime,
    roots: &ManagedRootStore,
) -> Result<FileBrowseProcessingReport, String> {
    let requests = agent
        .pending_file_browse_requests()
        .await
        .map_err(|error| error.to_string())?;

    let mut report = FileBrowseProcessingReport {
        inspected_count: requests.len(),
        completed_count: 0,
        failed_count: 0,
        results: Vec::new(),
    };

    for request in requests {
        let request_id = request.request_id.clone();
        match process_one(agent, roots, request).await {
            Ok(result) => {
                report.completed_count += 1;
                report.results.push(FileBrowseProcessingResult {
                    request_id,
                    status: FileBrowseProcessingStatus::Completed,
                    entry_count: result.entries.len(),
                    next_cursor: result.next_cursor,
                    message: None,
                });
            }
            Err(error) => {
                report.failed_count += 1;
                report.results.push(FileBrowseProcessingResult {
                    request_id,
                    status: FileBrowseProcessingStatus::Failed,
                    entry_count: 0,
                    next_cursor: None,
                    message: Some(error),
                });
            }
        }
    }

    Ok(report)
}

async fn process_one(
    agent: &AgentRuntime,
    roots: &ManagedRootStore,
    request: AgentFileBrowseRequest,
) -> Result<AgentFileBrowseResult, String> {
    let result = build_result_for_request(agent, roots, &request).await;
    match result {
        Ok(result) => {
            agent
                .complete_file_browse_request(request.request_id, result.clone())
                .await
                .map_err(|error| error.to_string())?;
            Ok(result)
        }
        Err(FailureResult { code, message }) => {
            agent
                .fail_file_browse_request(request.request_id, code)
                .await
                .map_err(|error| error.to_string())?;
            Err(message)
        }
    }
}

struct FailureResult {
    code: AgentFileBrowseFailureCode,
    message: String,
}

async fn build_result_for_request(
    agent: &AgentRuntime,
    roots: &ManagedRootStore,
    request: &AgentFileBrowseRequest,
) -> Result<AgentFileBrowseResult, FailureResult> {
    let room = agent
        .root_id_for_room(request.room_id.clone())
        .await
        .map_err(|error| FailureResult {
            code: AgentFileBrowseFailureCode::OutsideManagedRoot,
            message: error.to_string(),
        })?;
    let managed_root = roots
        .ensure_active_room_binding(&room.root_id, &request.room_id)
        .map_err(|error| FailureResult {
            code: AgentFileBrowseFailureCode::OutsideManagedRoot,
            message: error,
        })?;
    if !managed_root.enabled {
        return Err(FailureResult {
            code: AgentFileBrowseFailureCode::OutsideManagedRoot,
            message: format!("managed root is disabled: {}", managed_root.root_id),
        });
    }

    match request.query.as_deref() {
        Some(query) => build_search_result_page_limited(
            &managed_root.root,
            &request.relative_directory,
            query,
            request.search_scope,
            request.cursor.as_deref(),
            request.limit,
            &request.extensions,
        ),
        None => build_result_page_limited(
            &managed_root.root,
            &request.relative_directory,
            request.cursor.as_deref(),
            request.limit,
        ),
    }
    .map_err(|error| FailureResult {
        code: error.failure_code,
        message: error.message,
    })
}

#[derive(Debug, PartialEq, Eq)]
pub struct BuildFileBrowseError {
    pub failure_code: AgentFileBrowseFailureCode,
    pub message: String,
}

pub fn build_result_page(
    root: &str,
    relative_directory: &str,
    cursor: Option<&str>,
) -> Result<AgentFileBrowseResult, BuildFileBrowseError> {
    build_result_page_limited(root, relative_directory, cursor, PAGE_SIZE)
}

pub fn build_result_page_limited(
    root: &str,
    relative_directory: &str,
    cursor: Option<&str>,
    limit: usize,
) -> Result<AgentFileBrowseResult, BuildFileBrowseError> {
    validate_page_limit(limit)?;
    let offset = parse_cursor(cursor)?;
    let report = browse_root(root, Some(relative_directory)).map_err(map_browse_error)?;
    let entries = report
        .entries
        .into_iter()
        .filter(is_remotely_browsable)
        .collect::<Vec<_>>();

    if offset > entries.len() {
        return Err(BuildFileBrowseError {
            failure_code: AgentFileBrowseFailureCode::CursorInvalidated,
            message: "file browse cursor is no longer valid for this directory".to_string(),
        });
    }

    let page = entries
        .iter()
        .skip(offset)
        .take(limit)
        .map(remote_entry)
        .collect::<Vec<_>>();
    let next_offset = offset + page.len();
    let next_cursor = if next_offset < entries.len() {
        Some(format!("offset:{next_offset}"))
    } else {
        None
    };

    Ok(AgentFileBrowseResult {
        entries: page,
        next_cursor,
        desktop_generation: unix_ms().to_string(),
    })
}

pub fn build_search_result_page(
    root: &str,
    relative_directory: &str,
    query: &str,
    scope: AgentFileSearchScope,
    cursor: Option<&str>,
) -> Result<AgentFileBrowseResult, BuildFileBrowseError> {
    build_search_result_page_limited(
        root,
        relative_directory,
        query,
        scope,
        cursor,
        PAGE_SIZE,
        &[],
    )
}

pub fn build_search_result_page_limited(
    root: &str,
    relative_directory: &str,
    query: &str,
    scope: AgentFileSearchScope,
    cursor: Option<&str>,
    limit: usize,
    extensions: &[String],
) -> Result<AgentFileBrowseResult, BuildFileBrowseError> {
    validate_page_limit(limit)?;
    let fingerprint = search_fingerprint(relative_directory, query, scope, limit, extensions);
    let (expected_generation, offset) = parse_search_cursor(cursor, fingerprint)?;
    if !index_is_initialized(root).map_err(map_file_index_error)? {
        reindex_root(root).map_err(map_file_index_error)?;
    }
    let page = search_index_page_with_extensions(
        root,
        query,
        match scope {
            AgentFileSearchScope::CurrentDirectory => IndexedSearchScope::CurrentDirectory,
            AgentFileSearchScope::ManagedRoot => IndexedSearchScope::ManagedRoot,
        },
        relative_directory,
        expected_generation,
        offset,
        limit,
        extensions,
    )
    .map_err(map_file_index_error)?;
    let next_cursor = page.next_offset.map(|next_offset| {
        format!(
            "search:{fingerprint:016x}:{}:{next_offset}",
            page.generation
        )
    });
    let entries = page
        .entries
        .iter()
        .filter(|entry| is_remotely_browsable_name(&entry.name))
        .map(remote_search_entry)
        .collect();

    Ok(AgentFileBrowseResult {
        entries,
        next_cursor,
        desktop_generation: page.generation.to_string(),
    })
}

fn parse_cursor(cursor: Option<&str>) -> Result<usize, BuildFileBrowseError> {
    match cursor {
        None | Some("") => Ok(0),
        Some(raw) => {
            let raw_offset = raw.strip_prefix("offset:").unwrap_or(raw);
            raw_offset
                .parse::<usize>()
                .map_err(|_| BuildFileBrowseError {
                    failure_code: AgentFileBrowseFailureCode::CursorInvalidated,
                    message: "file browse cursor must be an offset cursor".to_string(),
                })
        }
    }
}

fn validate_page_limit(limit: usize) -> Result<(), BuildFileBrowseError> {
    if (1..=PAGE_SIZE).contains(&limit) {
        return Ok(());
    }
    Err(BuildFileBrowseError {
        failure_code: AgentFileBrowseFailureCode::CursorInvalidated,
        message: "file browse limit must be between 1 and 200".to_string(),
    })
}

fn parse_search_cursor(
    cursor: Option<&str>,
    fingerprint: u64,
) -> Result<(Option<u64>, usize), BuildFileBrowseError> {
    let Some(cursor) = cursor.filter(|cursor| !cursor.is_empty()) else {
        return Ok((None, 0));
    };
    let mut parts = cursor.split(':');
    let valid_prefix = parts.next() == Some("search");
    let cursor_fingerprint = parts
        .next()
        .and_then(|part| u64::from_str_radix(part, 16).ok());
    let generation = parts.next().and_then(|part| part.parse::<u64>().ok());
    let offset = parts.next().and_then(|part| part.parse::<usize>().ok());
    if !valid_prefix
        || cursor_fingerprint != Some(fingerprint)
        || generation.is_none()
        || offset.is_none()
        || parts.next().is_some()
    {
        return Err(BuildFileBrowseError {
            failure_code: AgentFileBrowseFailureCode::CursorInvalidated,
            message: "file search cursor does not match this query and scope".to_string(),
        });
    }
    Ok((generation, offset.unwrap_or_default()))
}

fn map_browse_error(error: BrowseError) -> BuildFileBrowseError {
    BuildFileBrowseError {
        failure_code: AgentFileBrowseFailureCode::OutsideManagedRoot,
        message: error.to_string(),
    }
}

fn map_file_index_error(error: FileIndexError) -> BuildFileBrowseError {
    let failure_code = match &error {
        FileIndexError::GenerationChanged { .. } | FileIndexError::InvalidSearch(_) => {
            AgentFileBrowseFailureCode::CursorInvalidated
        }
        _ => AgentFileBrowseFailureCode::OutsideManagedRoot,
    };
    BuildFileBrowseError {
        failure_code,
        message: error.to_string(),
    }
}

fn is_remotely_browsable(entry: &BrowseEntry) -> bool {
    is_remotely_browsable_name(&entry.name)
}

fn is_remotely_browsable_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    if name == ".mousekeeper" || name == ".mousekeeper_trash" {
        return false;
    }
    if name.ends_with(".tmp")
        || name.ends_with(".crdownload")
        || name.ends_with(".part")
        || name.ends_with(".lock")
        || name.ends_with(".download")
    {
        return false;
    }
    if name.contains("credential")
        || name.contains("password")
        || name.contains("secret")
        || name.contains("token")
        || name.ends_with(".key")
        || name.ends_with(".pem")
        || name.ends_with(".p12")
        || name.ends_with(".pfx")
    {
        return false;
    }
    true
}

fn remote_search_entry(entry: &IndexedSearchEntry) -> AgentFileBrowseEntry {
    AgentFileBrowseEntry {
        name: entry.name.clone(),
        relative_path: entry.relative_path.clone(),
        entry_type: if entry.is_dir {
            AgentFileBrowseEntryType::Directory
        } else {
            AgentFileBrowseEntryType::File
        },
        size_bytes: entry.size_bytes,
        modified_at: unix_ms_to_rfc3339(entry.modified_unix_ms),
        file_id: stable_file_id_parts(
            &entry.relative_path,
            entry.is_dir,
            entry.size_bytes,
            entry.modified_unix_ms,
        ),
    }
}

fn remote_entry(entry: &BrowseEntry) -> AgentFileBrowseEntry {
    AgentFileBrowseEntry {
        name: entry.name.clone(),
        relative_path: entry.path.clone(),
        entry_type: if entry.is_dir {
            AgentFileBrowseEntryType::Directory
        } else {
            AgentFileBrowseEntryType::File
        },
        size_bytes: entry.size_bytes,
        modified_at: unix_ms_to_rfc3339(entry.modified_unix_ms),
        file_id: stable_file_id(entry),
    }
}

fn stable_file_id(entry: &BrowseEntry) -> String {
    stable_file_id_parts(
        &entry.path,
        entry.is_dir,
        entry.size_bytes,
        entry.modified_unix_ms,
    )
}

fn stable_file_id_parts(
    relative_path: &str,
    is_dir: bool,
    size_bytes: Option<u64>,
    modified_unix_ms: Option<u128>,
) -> String {
    let modified = modified_unix_ms
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let size = size_bytes
        .map(|value| value.to_string())
        .unwrap_or_else(|| "dir".to_string());
    format!(
        "hm:{:016x}",
        fnv1a_64(format!("{relative_path}|{is_dir}|{size}|{modified}").as_bytes())
    )
}

fn search_fingerprint(
    relative_directory: &str,
    query: &str,
    scope: AgentFileSearchScope,
    limit: usize,
    extensions: &[String],
) -> u64 {
    let mut extensions = extensions
        .iter()
        .map(|extension| extension.to_ascii_lowercase())
        .collect::<Vec<_>>();
    extensions.sort();
    fnv1a_64(
        format!(
            "{}|{}|{:?}|{}|{}",
            relative_directory.replace('\\', "/"),
            query.to_lowercase(),
            scope,
            limit,
            extensions.join(",")
        )
        .as_bytes(),
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

    use super::{
        build_result_page, build_search_result_page, build_search_result_page_limited,
        unix_ms_to_rfc3339,
    };
    use crate::agent::{AgentFileBrowseFailureCode, AgentFileSearchScope};

    #[test]
    fn builds_relative_only_browse_page_and_filters_remote_unsafe_names() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("docs")).expect("docs");
        fs::write(root.join("visible.txt"), "hello").expect("visible");
        fs::write(root.join("download.crdownload"), "partial").expect("partial");
        fs::write(root.join("api-token.txt"), "secret").expect("token");

        let result =
            build_result_page(root.to_str().expect("root path"), "", None).expect("browse page");

        let paths = result
            .entries
            .iter()
            .map(|entry| entry.relative_path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(paths, vec!["docs", "visible.txt"]);
        assert!(paths
            .iter()
            .all(|path| !path.contains(root.to_string_lossy().as_ref())));
    }

    #[test]
    fn paginates_with_offset_cursor() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("root");
        for index in 0..205 {
            fs::write(root.join(format!("{index:03}.txt")), "x").expect("file");
        }

        let first = build_result_page(root.to_str().expect("root path"), "", None).expect("first");
        assert_eq!(first.entries.len(), 200);
        assert_eq!(first.next_cursor.as_deref(), Some("offset:200"));

        let second = build_result_page(
            root.to_str().expect("root path"),
            "",
            first.next_cursor.as_deref(),
        )
        .expect("second");
        assert_eq!(second.entries.len(), 5);
        assert!(second.next_cursor.is_none());
    }

    #[test]
    fn invalid_cursor_is_structured_failure() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("root");

        let error = build_result_page(root.to_str().expect("root path"), "", Some("bad"))
            .expect_err("cursor rejected");

        assert_eq!(
            error.failure_code,
            AgentFileBrowseFailureCode::CursorInvalidated
        );
    }

    #[test]
    fn traversal_is_rejected_before_escape() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("root");

        let error = build_result_page(root.to_str().expect("root path"), "../outside", None)
            .expect_err("traversal rejected");

        assert_eq!(
            error.failure_code,
            AgentFileBrowseFailureCode::OutsideManagedRoot
        );
    }

    #[test]
    fn unix_ms_format_is_contract_datetime() {
        assert_eq!(
            unix_ms_to_rfc3339(Some(1_704_067_200_123)),
            "2024-01-01T00:00:00.123Z"
        );
    }

    #[test]
    fn indexed_search_cursor_paginates_and_binds_query_scope_and_generation() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("root");
        for index in 0..205 {
            fs::write(root.join(format!("report-{index:03}.txt")), "x").expect("file");
        }

        let first = build_search_result_page(
            root.to_str().expect("root path"),
            "",
            "REPORT",
            AgentFileSearchScope::ManagedRoot,
            None,
        )
        .expect("first page");
        assert_eq!(first.entries.len(), 200);
        let cursor = first.next_cursor.as_deref().expect("next cursor");

        let second = build_search_result_page(
            root.to_str().expect("root path"),
            "",
            "report",
            AgentFileSearchScope::ManagedRoot,
            Some(cursor),
        )
        .expect("second page");
        assert_eq!(second.entries.len(), 5);
        assert!(second.next_cursor.is_none());

        let mismatched = build_search_result_page(
            root.to_str().expect("root path"),
            "",
            "invoice",
            AgentFileSearchScope::ManagedRoot,
            Some(cursor),
        )
        .expect_err("query fingerprint mismatch");
        assert_eq!(
            mismatched.failure_code,
            AgentFileBrowseFailureCode::CursorInvalidated
        );

        fs::write(root.join("report-new.txt"), "new").expect("new file");
        file_engine_cli::file_index::upsert_existing_file(&root, "report-new.txt")
            .expect("update index generation");
        let stale = build_search_result_page(
            root.to_str().expect("root path"),
            "",
            "report",
            AgentFileSearchScope::ManagedRoot,
            Some(cursor),
        )
        .expect_err("stale generation");
        assert_eq!(
            stale.failure_code,
            AgentFileBrowseFailureCode::CursorInvalidated
        );
    }

    #[test]
    fn indexed_search_honors_request_limit_and_extensions() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("root");
        for index in 0..20 {
            fs::write(root.join(format!("report-{index:02}.txt")), "x").expect("txt");
        }
        for index in 0..3 {
            fs::write(root.join(format!("report-{index:02}.pdf")), "x").expect("pdf");
        }

        let first = build_search_result_page_limited(
            root.to_str().expect("root path"),
            "",
            "report",
            AgentFileSearchScope::ManagedRoot,
            None,
            2,
            &[".pdf".to_string()],
        )
        .expect("filtered first page");
        assert_eq!(first.entries.len(), 2);
        assert!(first
            .entries
            .iter()
            .all(|entry| entry.relative_path.ends_with(".pdf")));
        let cursor = first.next_cursor.as_deref().expect("next cursor");

        let second = build_search_result_page_limited(
            root.to_str().expect("root path"),
            "",
            "report",
            AgentFileSearchScope::ManagedRoot,
            Some(cursor),
            2,
            &[".pdf".to_string()],
        )
        .expect("filtered second page");
        assert_eq!(second.entries.len(), 1);
        assert!(second.next_cursor.is_none());

        let mismatched_limit = build_search_result_page_limited(
            root.to_str().expect("root path"),
            "",
            "report",
            AgentFileSearchScope::ManagedRoot,
            Some(cursor),
            3,
            &[".pdf".to_string()],
        )
        .expect_err("search cursor must bind the request limit");
        assert_eq!(
            mismatched_limit.failure_code,
            AgentFileBrowseFailureCode::CursorInvalidated
        );
    }

    #[test]
    fn search_rebuilds_an_intentionally_cleared_nonzero_generation() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("root");
        fs::write(root.join("report.txt"), "report").expect("file");
        let indexed = file_engine_cli::file_index::reindex_root(&root).expect("index");
        file_engine_cli::file_index::clear_index(&root).expect("clear on disconnect");
        assert!(
            file_engine_cli::file_index::index_generation(&root).expect("generation")
                > indexed.generation
        );
        assert!(
            !file_engine_cli::file_index::index_is_initialized(&root).expect("initialized flag")
        );

        let result = build_search_result_page(
            root.to_str().expect("root path"),
            "",
            "report",
            AgentFileSearchScope::ManagedRoot,
            None,
        )
        .expect("search rebuilds cleared index");

        assert_eq!(result.entries.len(), 1);
        assert!(file_engine_cli::file_index::index_is_initialized(&root).expect("rebuilt flag"));
    }
}
