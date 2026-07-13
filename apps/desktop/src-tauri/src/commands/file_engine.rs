use std::fs;
use std::path::Path;

use file_engine_cli::analyzer::{analyze_root as analyze_file_root, AnalyzeReport};
use file_engine_cli::auto_approval::auto_approve_decisions;
use file_engine_cli::browse::{browse_root, BrowseReport};
use file_engine_cli::decision::{apply_decisions, DecisionApplication, DecisionEntry};
use file_engine_cli::execute::{execute_decision_application, ExecuteReport};
use file_engine_cli::file_index::{list_index, reindex_root, search_index, FileIndexReport};
use file_engine_cli::file_ops::{
    create_empty_file, rename_file as rename_file_in_root, CreateFileReport, RenameFileReport,
};
use file_engine_cli::journal::{
    read_operation_history, recover_journal as recover_journal_file, JournalRecoveryReport,
    OperationHistoryReport,
};
use file_engine_cli::path_guard::PathGuard;
use file_engine_cli::precondition::{precheck_proposals, PrecheckReport};
use file_engine_cli::proposal::{propose_for_root, ProposalReport};
use file_engine_cli::trash::{trash_file as trash_file_in_root, TrashReport};
use file_engine_cli::undo::{
    undo_operation as undo_file_operation, undo_root as undo_file_root, UndoReport,
};

use crate::agent::AgentRuntime;
use crate::cleanliness::{
    calculate_cleanliness_snapshot as calculate_cleanliness_snapshot_for_root, CleanlinessSnapshot,
};
use crate::storage::auto_approval::{
    AutoApprovalPolicyPatch, AutoApprovalPolicyRecord, AutoApprovalStore,
};
use crate::storage::managed_roots::{ManagedRoot, ManagedRootStatePatch, ManagedRootStore};
use crate::storage::watchers::WatcherStore;

const DEMO_ROOT_DIR_NAME: &str = "mousekeeper-ui-demo";

/// Outcome of unregistering (folder-level unpairing) a managed root. The local removal is always
/// authoritative once this returns Ok; `server_room_removed` reports whether the mobile-facing room
/// was also torn down, so the UI can tell the user when that step is still pending (e.g. offline).
#[derive(Clone, Debug, serde::Serialize, PartialEq, Eq)]
pub struct UnregisterManagedRootReport {
    pub root_id: String,
    pub server_room_removed: bool,
    pub server_message: Option<String>,
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn register_managed_root(
    path: String,
    window: tauri::Window,
    store: tauri::State<'_, ManagedRootStore>,
) -> Result<ManagedRoot, String> {
    crate::commands::permissions::require_main_window(&window)?;
    register_managed_root_in_store(path, &store)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn register_managed_root(path: String) -> Result<ManagedRoot, String> {
    register_managed_root_without_store(path)
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn list_managed_roots(
    store: tauri::State<'_, ManagedRootStore>,
) -> Result<Vec<ManagedRoot>, String> {
    store.list()
}

#[cfg(not(feature = "tauri-commands"))]
pub fn list_managed_roots(store: &ManagedRootStore) -> Result<Vec<ManagedRoot>, String> {
    store.list()
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn update_managed_root_state(
    root_id: String,
    patch: ManagedRootStatePatch,
    window: tauri::Window,
    store: tauri::State<'_, ManagedRootStore>,
    watchers: tauri::State<'_, WatcherStore>,
) -> Result<ManagedRoot, String> {
    crate::commands::permissions::require_main_window(&window)?;
    update_managed_root_state_impl(root_id, patch, &store, Some(&watchers))
}

#[cfg(not(feature = "tauri-commands"))]
pub fn update_managed_root_state(
    root_id: String,
    patch: ManagedRootStatePatch,
    store: &ManagedRootStore,
) -> Result<ManagedRoot, String> {
    update_managed_root_state_impl(root_id, patch, store, None)
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn unregister_managed_root(
    root_id: String,
    window: tauri::Window,
    runtime: tauri::State<'_, AgentRuntime>,
    store: tauri::State<'_, ManagedRootStore>,
    auto_approval: tauri::State<'_, AutoApprovalStore>,
    watchers: tauri::State<'_, WatcherStore>,
) -> Result<UnregisterManagedRootReport, String> {
    crate::commands::permissions::require_main_window(&window)?;
    // Stop any live watcher before the root disappears so it cannot fire against a folder we no
    // longer manage. This is best-effort: a root that was not being watched simply returns false.
    let _ = watchers.stop(&root_id);
    unregister_managed_root_impl(root_id, &runtime, &store, &auto_approval).await
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn unregister_managed_root(
    root_id: String,
    runtime: &AgentRuntime,
    store: &ManagedRootStore,
    auto_approval: &AutoApprovalStore,
) -> Result<UnregisterManagedRootReport, String> {
    unregister_managed_root_impl(root_id, runtime, store, auto_approval).await
}

async fn unregister_managed_root_impl(
    root_id: String,
    runtime: &AgentRuntime,
    store: &ManagedRootStore,
    auto_approval: &AutoApprovalStore,
) -> Result<UnregisterManagedRootReport, String> {
    // The root must be registered locally; this surfaces a clear error otherwise before any
    // server-side teardown runs.
    store.get(&root_id)?;

    // Best-effort: remove the mobile-facing room so the phone stops listing this folder. Offline
    // or an already-removed room must not block local removal, so any failure is captured in the
    // report instead of aborting.
    let mut server_room_removed = false;
    let mut server_message = None;
    match runtime.list_rooms().await {
        Ok(rooms) => match rooms.into_iter().find(|room| room.root_id == root_id) {
            Some(room) => match runtime.remove_room(room.room_id).await {
                Ok(()) => server_room_removed = true,
                Err(error) => server_message = Some(error.to_string()),
            },
            // No room was ever synced for this root, so there is nothing to tear down remotely.
            None => server_room_removed = true,
        },
        Err(error) => server_message = Some(error.to_string()),
    }

    // Drop the local auto-approval policy, then the managed root itself (authoritative). Files on
    // disk are never touched by unregistering.
    let _ = auto_approval.remove(&root_id);
    store.remove(&root_id)?;

    Ok(UnregisterManagedRootReport {
        root_id,
        server_room_removed,
        server_message,
    })
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn prepare_demo_root(window: tauri::Window) -> Result<String, String> {
    crate::commands::permissions::require_main_window(&window)?;
    prepare_demo_root_impl()
}

#[cfg(not(feature = "tauri-commands"))]
pub fn prepare_demo_root() -> Result<String, String> {
    prepare_demo_root_impl()
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn analyze_root(
    root_id: String,
    store: tauri::State<'_, ManagedRootStore>,
) -> Result<AnalyzeReport, String> {
    let root = resolve_root_id(&store, &root_id)?;
    analyze_root_impl(root)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn analyze_root(root_id: String, store: &ManagedRootStore) -> Result<AnalyzeReport, String> {
    let root = resolve_root_id(store, &root_id)?;
    analyze_root_impl(root)
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn browse_root_tree(
    root_id: String,
    path: Option<String>,
    store: tauri::State<'_, ManagedRootStore>,
) -> Result<BrowseReport, String> {
    let root = resolve_root_id(&store, &root_id)?;
    browse_root_tree_impl(root, path)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn browse_root_tree(
    root_id: String,
    path: Option<String>,
    store: &ManagedRootStore,
) -> Result<BrowseReport, String> {
    let root = resolve_root_id(store, &root_id)?;
    browse_root_tree_impl(root, path)
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn reindex_managed_root(
    root_id: String,
    store: tauri::State<'_, ManagedRootStore>,
) -> Result<FileIndexReport, String> {
    let root = resolve_root_id(&store, &root_id)?;
    reindex_managed_root_impl(root)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn reindex_managed_root(
    root_id: String,
    store: &ManagedRootStore,
) -> Result<FileIndexReport, String> {
    let root = resolve_root_id(store, &root_id)?;
    reindex_managed_root_impl(root)
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn search_managed_root(
    root_id: String,
    query: String,
    store: tauri::State<'_, ManagedRootStore>,
) -> Result<FileIndexReport, String> {
    let root = resolve_root_id(&store, &root_id)?;
    search_managed_root_impl(root, query)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn search_managed_root(
    root_id: String,
    query: String,
    store: &ManagedRootStore,
) -> Result<FileIndexReport, String> {
    let root = resolve_root_id(store, &root_id)?;
    search_managed_root_impl(root, query)
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn propose_file_changes(
    root_id: String,
    store: tauri::State<'_, ManagedRootStore>,
) -> Result<ProposalReport, String> {
    let root = resolve_root_id(&store, &root_id)?;
    propose_file_changes_impl(root)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn propose_file_changes(
    root_id: String,
    store: &ManagedRootStore,
) -> Result<ProposalReport, String> {
    let root = resolve_root_id(store, &root_id)?;
    propose_file_changes_impl(root)
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn calculate_cleanliness_snapshot(
    root_id: String,
    store: tauri::State<'_, ManagedRootStore>,
) -> Result<CleanlinessSnapshot, String> {
    let root = resolve_root_id(&store, &root_id)?;
    calculate_cleanliness_snapshot_impl(root)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn calculate_cleanliness_snapshot(
    root_id: String,
    store: &ManagedRootStore,
) -> Result<CleanlinessSnapshot, String> {
    let root = resolve_root_id(store, &root_id)?;
    calculate_cleanliness_snapshot_impl(root)
}

/// Local validation of an AI/rule-draft (plan item 12). Strictly parses and validates a candidate
/// Rule DSL draft without touching the filesystem — B's desktop AI code can call this to reject a
/// bad draft before it ever enters the command/proposal pipeline. Validation only: it never reads
/// the disk, computes targets, or mutates anything.
#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn validate_rule_draft(draft: serde_json::Value) -> Result<(), String> {
    validate_rule_draft_impl(draft)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn validate_rule_draft(draft: serde_json::Value) -> Result<(), String> {
    validate_rule_draft_impl(draft)
}

fn validate_rule_draft_impl(draft: serde_json::Value) -> Result<(), String> {
    let rule_set: file_engine_cli::rules::RuleSet = serde_json::from_value(draft)
        .map_err(|error| format!("invalid rule draft shape: {error}"))?;
    rule_set.validate().map_err(command_error)
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn get_auto_approval_policy(
    root_id: String,
    roots: tauri::State<'_, ManagedRootStore>,
    policies: tauri::State<'_, AutoApprovalStore>,
) -> Result<AutoApprovalPolicyRecord, String> {
    get_auto_approval_policy_impl(root_id, &roots, &policies)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn get_auto_approval_policy(
    root_id: String,
    roots: &ManagedRootStore,
    policies: &AutoApprovalStore,
) -> Result<AutoApprovalPolicyRecord, String> {
    get_auto_approval_policy_impl(root_id, roots, policies)
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn update_auto_approval_policy(
    root_id: String,
    patch: AutoApprovalPolicyPatch,
    window: tauri::Window,
    roots: tauri::State<'_, ManagedRootStore>,
    policies: tauri::State<'_, AutoApprovalStore>,
) -> Result<AutoApprovalPolicyRecord, String> {
    crate::commands::permissions::require_main_window(&window)?;
    update_auto_approval_policy_impl(root_id, patch, &roots, &policies)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn update_auto_approval_policy(
    root_id: String,
    patch: AutoApprovalPolicyPatch,
    roots: &ManagedRootStore,
    policies: &AutoApprovalStore,
) -> Result<AutoApprovalPolicyRecord, String> {
    update_auto_approval_policy_impl(root_id, patch, roots, policies)
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn auto_approve_file_changes(
    root_id: String,
    proposal: ProposalReport,
    window: tauri::Window,
    roots: tauri::State<'_, ManagedRootStore>,
    policies: tauri::State<'_, AutoApprovalStore>,
) -> Result<Vec<DecisionEntry>, String> {
    crate::commands::permissions::require_main_window(&window)?;
    auto_approve_file_changes_impl(root_id, proposal, &roots, &policies)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn auto_approve_file_changes(
    root_id: String,
    proposal: ProposalReport,
    roots: &ManagedRootStore,
    policies: &AutoApprovalStore,
) -> Result<Vec<DecisionEntry>, String> {
    auto_approve_file_changes_impl(root_id, proposal, roots, policies)
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn precheck_file_changes(
    root_id: String,
    proposal: ProposalReport,
    decisions: Vec<DecisionEntry>,
    store: tauri::State<'_, ManagedRootStore>,
) -> Result<PrecheckReport, String> {
    let root = resolve_root_id(&store, &root_id)?;
    precheck_file_changes_impl(root, proposal, decisions)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn precheck_file_changes(
    root_id: String,
    proposal: ProposalReport,
    decisions: Vec<DecisionEntry>,
    store: &ManagedRootStore,
) -> Result<PrecheckReport, String> {
    let root = resolve_root_id(store, &root_id)?;
    precheck_file_changes_impl(root, proposal, decisions)
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn execute_file_changes(
    root_id: String,
    proposal: ProposalReport,
    decisions: Vec<DecisionEntry>,
    window: tauri::Window,
    store: tauri::State<'_, ManagedRootStore>,
) -> Result<ExecuteReport, String> {
    crate::commands::permissions::require_main_window(&window)?;
    let root = resolve_root_id(&store, &root_id)?;
    execute_file_changes_impl(root, proposal, decisions)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn execute_file_changes(
    root_id: String,
    proposal: ProposalReport,
    decisions: Vec<DecisionEntry>,
    store: &ManagedRootStore,
) -> Result<ExecuteReport, String> {
    let root = resolve_root_id(store, &root_id)?;
    execute_file_changes_impl(root, proposal, decisions)
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn trash_file(
    root_id: String,
    path: String,
    window: tauri::Window,
    store: tauri::State<'_, ManagedRootStore>,
) -> Result<TrashReport, String> {
    crate::commands::permissions::require_main_window(&window)?;
    let root = resolve_root_id(&store, &root_id)?;
    trash_file_impl(root, path)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn trash_file(
    root_id: String,
    path: String,
    store: &ManagedRootStore,
) -> Result<TrashReport, String> {
    let root = resolve_root_id(store, &root_id)?;
    trash_file_impl(root, path)
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn create_file(
    root_id: String,
    path: String,
    window: tauri::Window,
    store: tauri::State<'_, ManagedRootStore>,
) -> Result<CreateFileReport, String> {
    crate::commands::permissions::require_main_window(&window)?;
    let root = resolve_root_id(&store, &root_id)?;
    create_file_impl(root, path)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn create_file(
    root_id: String,
    path: String,
    store: &ManagedRootStore,
) -> Result<CreateFileReport, String> {
    let root = resolve_root_id(store, &root_id)?;
    create_file_impl(root, path)
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn rename_file(
    root_id: String,
    path: String,
    new_name: String,
    window: tauri::Window,
    store: tauri::State<'_, ManagedRootStore>,
) -> Result<RenameFileReport, String> {
    crate::commands::permissions::require_main_window(&window)?;
    let root = resolve_root_id(&store, &root_id)?;
    rename_file_impl(root, path, new_name)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn rename_file(
    root_id: String,
    path: String,
    new_name: String,
    store: &ManagedRootStore,
) -> Result<RenameFileReport, String> {
    let root = resolve_root_id(store, &root_id)?;
    rename_file_impl(root, path, new_name)
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn undo_last_file_operation(
    root_id: String,
    window: tauri::Window,
    store: tauri::State<'_, ManagedRootStore>,
) -> Result<UndoReport, String> {
    crate::commands::permissions::require_main_window(&window)?;
    let root = resolve_root_id(&store, &root_id)?;
    undo_last_file_operation_impl(root)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn undo_last_file_operation(
    root_id: String,
    store: &ManagedRootStore,
) -> Result<UndoReport, String> {
    let root = resolve_root_id(store, &root_id)?;
    undo_last_file_operation_impl(root)
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn undo_operation(
    root_id: String,
    operation_id: String,
    window: tauri::Window,
    store: tauri::State<'_, ManagedRootStore>,
) -> Result<UndoReport, String> {
    crate::commands::permissions::require_main_window(&window)?;
    let root = resolve_root_id(&store, &root_id)?;
    undo_operation_impl(root, operation_id)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn undo_operation(
    root_id: String,
    operation_id: String,
    store: &ManagedRootStore,
) -> Result<UndoReport, String> {
    let root = resolve_root_id(store, &root_id)?;
    undo_operation_impl(root, operation_id)
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn list_operation_history(
    root_id: String,
    store: tauri::State<'_, ManagedRootStore>,
) -> Result<OperationHistoryReport, String> {
    let root = resolve_root_id(&store, &root_id)?;
    list_operation_history_impl(root)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn list_operation_history(
    root_id: String,
    store: &ManagedRootStore,
) -> Result<OperationHistoryReport, String> {
    let root = resolve_root_id(store, &root_id)?;
    list_operation_history_impl(root)
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn recover_journal(
    root_id: String,
    window: tauri::Window,
    store: tauri::State<'_, ManagedRootStore>,
) -> Result<JournalRecoveryReport, String> {
    crate::commands::permissions::require_main_window(&window)?;
    let root = resolve_root_id(&store, &root_id)?;
    recover_journal_impl(root)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn recover_journal(
    root_id: String,
    store: &ManagedRootStore,
) -> Result<JournalRecoveryReport, String> {
    let root = resolve_root_id(store, &root_id)?;
    recover_journal_impl(root)
}

fn register_managed_root_without_store(path: String) -> Result<ManagedRoot, String> {
    let guard = PathGuard::new(path).map_err(command_error)?;
    let root = guard.root();

    Ok(ManagedRoot::new(
        managed_root_id(&root.display().to_string()),
        root.display().to_string(),
        root.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("managed root")
            .to_string(),
    ))
}

#[cfg(any(feature = "tauri-commands", test))]
fn register_managed_root_in_store(
    path: String,
    store: &ManagedRootStore,
) -> Result<ManagedRoot, String> {
    let managed = register_managed_root_without_store(path)?;
    store.upsert(managed)
}

fn update_managed_root_state_impl(
    root_id: String,
    patch: ManagedRootStatePatch,
    store: &ManagedRootStore,
    watchers: Option<&WatcherStore>,
) -> Result<ManagedRoot, String> {
    let should_stop_watcher = patch.enabled == Some(false);
    let updated = store.update_state(&root_id, patch)?;
    if should_stop_watcher {
        if let Some(watchers) = watchers {
            let _ = watchers.stop(&root_id)?;
        }
    }

    Ok(updated)
}

fn get_auto_approval_policy_impl(
    root_id: String,
    roots: &ManagedRootStore,
    policies: &AutoApprovalStore,
) -> Result<AutoApprovalPolicyRecord, String> {
    roots.get(&root_id)?;
    policies.get_or_default(&root_id)
}

fn update_auto_approval_policy_impl(
    root_id: String,
    patch: AutoApprovalPolicyPatch,
    roots: &ManagedRootStore,
    policies: &AutoApprovalStore,
) -> Result<AutoApprovalPolicyRecord, String> {
    roots.get(&root_id)?;
    policies.patch(&root_id, patch)
}

fn auto_approve_file_changes_impl(
    root_id: String,
    proposal: ProposalReport,
    roots: &ManagedRootStore,
    policies: &AutoApprovalStore,
) -> Result<Vec<DecisionEntry>, String> {
    let root = resolve_root_id(roots, &root_id)?;
    if proposal.root != root {
        return Err("proposal root does not match the selected managed root".to_string());
    }

    let policy = policies.get_or_default(&root_id)?.to_engine_policy();
    auto_approve_decisions(&proposal, &policy).map_err(command_error)
}

fn prepare_demo_root_impl() -> Result<String, String> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("test-fixtures")
        .join("file-trees")
        .join("ui-demo")
        .canonicalize()
        .map_err(|error| format!("cannot locate ui-demo fixture: {error}"))?;
    let target = std::env::temp_dir().join(DEMO_ROOT_DIR_NAME);

    if target.exists() {
        fs::remove_dir_all(&target)
            .map_err(|error| format!("cannot reset demo root {}: {error}", target.display()))?;
    }

    copy_demo_tree(&source, &target)?;

    Ok(target.display().to_string())
}

fn copy_demo_tree(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target)
        .map_err(|error| format!("cannot create demo root {}: {error}", target.display()))?;

    for entry in fs::read_dir(source)
        .map_err(|error| format!("cannot read demo fixture {}: {error}", source.display()))?
    {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else {
            continue;
        };

        if matches!(name_str, ".mousekeeper" | ".mousekeeper_trash") {
            continue;
        }

        let destination = target.join(&name);
        let metadata = entry.metadata().map_err(|error| error.to_string())?;
        if metadata.is_dir() {
            copy_demo_tree(&path, &destination)?;
        } else if metadata.is_file() {
            fs::copy(&path, &destination).map_err(|error| {
                format!(
                    "cannot copy demo file {} to {}: {error}",
                    path.display(),
                    destination.display()
                )
            })?;
        }
    }

    Ok(())
}

fn analyze_root_impl(root: String) -> Result<AnalyzeReport, String> {
    analyze_file_root(root).map_err(command_error)
}

fn browse_root_tree_impl(root: String, path: Option<String>) -> Result<BrowseReport, String> {
    browse_root(root, path.as_deref()).map_err(command_error)
}

fn reindex_managed_root_impl(root: String) -> Result<FileIndexReport, String> {
    reindex_root(root).map_err(command_error)
}

fn search_managed_root_impl(root: String, query: String) -> Result<FileIndexReport, String> {
    if query.trim().is_empty() {
        list_index(root).map_err(command_error)
    } else {
        search_index(root, &query).map_err(command_error)
    }
}

fn propose_file_changes_impl(root: String) -> Result<ProposalReport, String> {
    propose_for_root(root).map_err(command_error)
}

fn calculate_cleanliness_snapshot_impl(root: String) -> Result<CleanlinessSnapshot, String> {
    calculate_cleanliness_snapshot_for_root(root)
}

fn precheck_file_changes_impl(
    root: String,
    proposal: ProposalReport,
    decisions: Vec<DecisionEntry>,
) -> Result<PrecheckReport, String> {
    apply_decisions_for_command(proposal, decisions).and_then(|application| {
        precheck_proposals(root, application.approved).map_err(command_error)
    })
}

fn execute_file_changes_impl(
    root: String,
    proposal: ProposalReport,
    decisions: Vec<DecisionEntry>,
) -> Result<ExecuteReport, String> {
    apply_decisions_for_command(proposal, decisions).and_then(|application| {
        execute_decision_application(root, application).map_err(command_error)
    })
}

fn trash_file_impl(root: String, path: String) -> Result<TrashReport, String> {
    trash_file_in_root(root, path).map_err(command_error)
}

fn create_file_impl(root: String, path: String) -> Result<CreateFileReport, String> {
    create_empty_file(root, path).map_err(command_error)
}

fn rename_file_impl(
    root: String,
    path: String,
    new_name: String,
) -> Result<RenameFileReport, String> {
    rename_file_in_root(root, path, new_name).map_err(command_error)
}

fn undo_last_file_operation_impl(root: String) -> Result<UndoReport, String> {
    undo_file_root(root).map_err(command_error)
}

fn undo_operation_impl(root: String, operation_id: String) -> Result<UndoReport, String> {
    undo_file_operation(root, operation_id).map_err(command_error)
}

fn list_operation_history_impl(root: String) -> Result<OperationHistoryReport, String> {
    read_operation_history(root).map_err(command_error)
}

fn recover_journal_impl(root: String) -> Result<JournalRecoveryReport, String> {
    recover_journal_file(root).map_err(command_error)
}

fn apply_decisions_for_command(
    proposal: ProposalReport,
    decisions: Vec<DecisionEntry>,
) -> Result<DecisionApplication, String> {
    if decisions.is_empty() {
        return Ok(DecisionApplication {
            approved: proposal,
            rejected: Vec::new(),
        });
    }

    apply_decisions(proposal, &decisions).map_err(command_error)
}

fn command_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn resolve_root_id(store: &ManagedRootStore, root_id: &str) -> Result<String, String> {
    Ok(store.get(root_id)?.root)
}

fn managed_root_id(root: &str) -> String {
    format!("root:{:016x}", fnv1a64(root.as_bytes()))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325;

    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }

    hash
}

// These tests exercise the `not(feature = "tauri-commands")` variants above, which take plain
// `&Store` arguments so they are callable without a live Tauri `State`/`AppHandle`. The
// `tauri-commands` variants need a running app to construct those arguments and are not
// unit-testable this way, so this module only compiles for the CLI/no-tauri-commands build.
#[cfg(all(test, not(feature = "tauri-commands")))]
mod tests {
    use std::fs;

    use file_engine_cli::decision::{Decision, DecisionEntry};
    use tempfile::tempdir;

    use crate::storage::auto_approval::{AutoApprovalPolicyPatch, AutoApprovalStore};
    use crate::storage::managed_roots::ManagedRootStore;

    use super::{
        auto_approve_file_changes, browse_root_tree, create_file, execute_file_changes,
        get_auto_approval_policy, list_managed_roots, list_operation_history,
        precheck_file_changes, prepare_demo_root, propose_file_changes, recover_journal,
        register_managed_root, register_managed_root_in_store, reindex_managed_root, rename_file,
        search_managed_root, trash_file, undo_operation, update_auto_approval_policy,
        validate_rule_draft,
    };

    #[test]
    fn validate_rule_draft_accepts_valid_and_rejects_invalid() {
        // A well-formed "clean up PDFs" draft passes local validation with no filesystem access.
        validate_rule_draft(serde_json::json!({
            "version": 1,
            "rules": [
                { "id": "pdfs", "when": { "extension_in": ["pdf"] }, "then": { "trash": true } }
            ]
        }))
        .expect("valid draft accepted");

        // Unknown field -> rejected at parse (strict schema).
        assert!(validate_rule_draft(serde_json::json!({
            "version": 1,
            "rules": [{ "id": "x", "when": {}, "then": { "nope": true } }]
        }))
        .is_err());

        // Well-formed but a rule with no condition ("match everything") -> rejected by validate().
        assert!(validate_rule_draft(serde_json::json!({
            "version": 1,
            "rules": [{ "id": "x", "when": {}, "then": { "trash": true } }]
        }))
        .is_err());
    }

    #[test]
    fn register_managed_root_returns_canonical_directory() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("create root");

        let managed =
            register_managed_root(root.display().to_string()).expect("register managed root");

        assert_eq!(
            managed.root,
            root.canonicalize()
                .expect("canonical")
                .display()
                .to_string()
        );
        assert!(managed.root_id.starts_with("root:"));
        assert_eq!(managed.display_name, "root");
    }

    #[test]
    fn register_managed_root_stores_root_for_later_listing() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("create root");
        let store = ManagedRootStore::default();

        let managed = register_managed_root_in_store(root.display().to_string(), &store)
            .expect("register stored root");
        let roots = list_managed_roots(&store).expect("list roots");

        assert_eq!(roots, vec![managed]);
    }

    #[test]
    fn register_managed_root_rejects_missing_directory() {
        let temp = tempdir().expect("tempdir");
        let missing = temp.path().join("missing");

        let error =
            register_managed_root(missing.display().to_string()).expect_err("reject missing root");

        assert!(!error.is_empty());
    }

    #[test]
    fn register_managed_root_rejects_parent_child_overlap() {
        let temp = tempdir().expect("tempdir");
        let parent = temp.path().join("parent");
        let child = parent.join("child");
        fs::create_dir_all(&child).expect("create nested roots");
        let store = ManagedRootStore::default();

        register_managed_root_in_store(parent.display().to_string(), &store)
            .expect("register parent");
        let error = register_managed_root_in_store(child.display().to_string(), &store)
            .expect_err("reject child root");

        assert!(error.contains("parent root"));
    }

    #[test]
    fn prepare_demo_root_uses_temp_copy_without_runtime_state() {
        let path = prepare_demo_root().expect("prepare demo");
        let root = std::path::PathBuf::from(path);

        assert!(root.starts_with(std::env::temp_dir()));
        assert!(root.join("documents").join("note.md").exists());
        assert!(!root.join(".mousekeeper").exists());
        assert!(!root.join(".mousekeeper_trash").exists());
    }

    #[test]
    fn command_flow_moves_only_approved_items_and_undoes_them() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        fs::write(root.join("inbox").join("photo.png"), "png").expect("write photo");

        let store = ManagedRootStore::default();
        let managed = register_managed_root_in_store(root.display().to_string(), &store)
            .expect("register managed root");
        let proposal =
            propose_file_changes(managed.root_id.clone(), &store).expect("propose file changes");
        let note_id = proposal
            .proposals
            .iter()
            .find(|proposal| proposal.from == "inbox/note.md")
            .expect("note proposal")
            .proposal_id
            .clone();
        let decisions = vec![DecisionEntry {
            proposal_id: note_id,
            decision: Decision::Approved,
            reason: None,
        }];

        let precheck = precheck_file_changes(
            managed.root_id.clone(),
            proposal.clone(),
            decisions.clone(),
            &store,
        )
        .expect("precheck file changes");
        assert_eq!(precheck.checks.len(), 1);

        let execute = execute_file_changes(managed.root_id.clone(), proposal, decisions, &store)
            .expect("execute");
        assert_eq!(execute.executed_count, 1);
        assert!(root.join("documents").join("note.md").exists());
        assert!(root.join("inbox").join("photo.png").exists());

        let history = list_operation_history(managed.root_id.clone(), &store).expect("history");
        assert_eq!(history.operations.len(), 1);
        assert!(history.operations[0].can_undo);

        let undo = undo_operation(
            managed.root_id.clone(),
            history.operations[0].operation_id.clone(),
            &store,
        )
        .expect("undo selected operation");
        assert_eq!(undo.undone_count, 1);
        assert!(root.join("inbox").join("note.md").exists());

        let history = list_operation_history(managed.root_id, &store).expect("history");
        assert!(!history.operations[0].can_undo);
    }

    #[test]
    fn auto_approval_policy_is_root_scoped_and_generates_decisions() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::create_dir_all(root.join(".mousekeeper")).expect("create state dir");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        fs::write(root.join("inbox").join("noise.tmp"), "noise").expect("write temp");
        fs::write(
            root.join(".mousekeeper").join("rules.json"),
            r#"{"version":1,"rules":[{"id":"temp-trash","when":{"name_matches":"*.tmp"},"then":{"trash":true}}]}"#,
        )
        .expect("write rules");

        let roots = ManagedRootStore::default();
        let policies = AutoApprovalStore::default();
        let managed = register_managed_root_in_store(root.display().to_string(), &roots)
            .expect("register managed root");
        let default_policy =
            get_auto_approval_policy(managed.root_id.clone(), &roots, &policies).expect("policy");
        assert!(!default_policy.enabled);

        update_auto_approval_policy(
            managed.root_id.clone(),
            AutoApprovalPolicyPatch {
                enabled: Some(true),
                allowed_actions: Some(vec![file_engine_cli::proposal::ProposalAction::Trash]),
                max_files_per_run: Some(5),
                expires_unix_ms: None,
            },
            &roots,
            &policies,
        )
        .expect("enable policy");

        let proposal =
            propose_file_changes(managed.root_id.clone(), &roots).expect("propose file changes");
        let decisions =
            auto_approve_file_changes(managed.root_id.clone(), proposal.clone(), &roots, &policies)
                .expect("auto decisions");

        assert_eq!(decisions.len(), 1);
        let approved = proposal
            .proposals
            .iter()
            .find(|proposal| proposal.proposal_id == decisions[0].proposal_id)
            .expect("approved proposal");
        assert_eq!(
            approved.action,
            file_engine_cli::proposal::ProposalAction::Trash
        );
    }

    #[test]
    fn browse_root_tree_lists_directory_level_relative_to_root() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");

        let store = ManagedRootStore::default();
        let managed = register_managed_root_in_store(root.display().to_string(), &store)
            .expect("register managed root");

        let top_level =
            browse_root_tree(managed.root_id.clone(), None, &store).expect("browse root");
        assert_eq!(top_level.entries.len(), 1);
        assert_eq!(top_level.entries[0].name, "inbox");
        assert!(top_level.entries[0].is_dir);

        let inbox = browse_root_tree(managed.root_id, Some("inbox".to_string()), &store)
            .expect("browse inbox");
        assert_eq!(inbox.entries.len(), 1);
        assert_eq!(inbox.entries[0].path, "inbox/note.md");
        assert!(!inbox.entries[0].is_dir);
    }

    #[test]
    fn trash_file_command_moves_file_to_recoverable_trash() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("noise.tmp"), "noise").expect("write temp");

        let store = ManagedRootStore::default();
        let managed = register_managed_root_in_store(root.display().to_string(), &store)
            .expect("register managed root");

        let report = trash_file(
            managed.root_id.clone(),
            "inbox/noise.tmp".to_string(),
            &store,
        )
        .expect("trash file");
        let history = list_operation_history(managed.root_id, &store).expect("history");

        assert!(!root.join("inbox").join("noise.tmp").exists());
        assert!(root.join(&report.trashed_path).exists());
        assert_eq!(history.operations.len(), 1);
        assert_eq!(
            history.operations[0].action,
            file_engine_cli::journal::JournalAction::Trash
        );
    }

    #[test]
    fn create_and_rename_file_commands_stay_inside_root() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("notes")).expect("create notes");

        let store = ManagedRootStore::default();
        let managed = register_managed_root_in_store(root.display().to_string(), &store)
            .expect("register managed root");

        let created = create_file(
            managed.root_id.clone(),
            "notes/draft.txt".to_string(),
            &store,
        )
        .expect("create file");
        let renamed = rename_file(
            managed.root_id.clone(),
            "notes/draft.txt".to_string(),
            "final.txt".to_string(),
            &store,
        )
        .expect("rename file");
        let history = list_operation_history(managed.root_id, &store).expect("history");

        assert_eq!(created.created_path, "notes/draft.txt");
        assert_eq!(renamed.to, "notes/final.txt");
        assert!(!root.join("notes").join("draft.txt").exists());
        assert!(root.join("notes").join("final.txt").exists());
        assert_eq!(
            history.operations[0].action,
            file_engine_cli::journal::JournalAction::Move
        );
    }

    #[test]
    fn history_reports_corruption_and_recover_journal_clears_it() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");

        let store = ManagedRootStore::default();
        let managed = register_managed_root_in_store(root.display().to_string(), &store)
            .expect("register managed root");
        let proposal =
            propose_file_changes(managed.root_id.clone(), &store).expect("propose file changes");
        let decisions = vec![DecisionEntry {
            proposal_id: proposal.proposals[0].proposal_id.clone(),
            decision: Decision::Approved,
            reason: None,
        }];
        execute_file_changes(managed.root_id.clone(), proposal, decisions, &store)
            .expect("execute");

        // Corrupt the SQLite-backed journal by inserting an unrecognized row, the analog of
        // the old "append a bad JSONL line".
        let pool = file_engine_cli::db::open_root_db(std::path::Path::new(&managed.root))
            .expect("open journal db");
        file_engine_cli::db::block_on(async {
            sqlx::query(
                "INSERT INTO operation_journal
                    (operation_id, status, action, from_path, to_path, created_unix_ms)
                 VALUES ('op-bad', 'garbage', 'move', 'a', 'b', 1)",
            )
            .execute(&pool)
            .await
            .expect("insert bad row");
        });

        let history = list_operation_history(managed.root_id.clone(), &store)
            .expect("history tolerates corruption");
        assert!(history.corruption.is_some());
        assert_eq!(history.operations.len(), 1);

        recover_journal(managed.root_id.clone(), &store).expect("recover journal");

        let history =
            list_operation_history(managed.root_id, &store).expect("history after recovery");
        assert!(history.corruption.is_none());
        assert!(history.operations.is_empty());
    }

    #[test]
    fn reindex_then_search_finds_indexed_files() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        fs::write(root.join("inbox").join("photo.png"), "png").expect("write photo");

        let store = ManagedRootStore::default();
        let managed = register_managed_root_in_store(root.display().to_string(), &store)
            .expect("register managed root");

        let indexed = reindex_managed_root(managed.root_id.clone(), &store).expect("reindex");
        assert_eq!(indexed.files.len(), 2);

        let hits = search_managed_root(managed.root_id.clone(), "note".to_string(), &store)
            .expect("search");
        assert_eq!(hits.files.len(), 1);
        assert_eq!(hits.files[0].relative_path, "inbox/note.md");

        // Empty query lists everything currently indexed.
        let all = search_managed_root(managed.root_id, String::new(), &store).expect("list");
        assert_eq!(all.files.len(), 2);
    }

    #[test]
    fn file_commands_reject_unknown_root_id() {
        let store = ManagedRootStore::default();

        let error = propose_file_changes("root:missing".to_string(), &store)
            .expect_err("reject unknown root id");

        assert!(error.contains("not registered"));
    }
}
