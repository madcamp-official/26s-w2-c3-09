use file_engine_cli::analyzer::{analyze_root as analyze_file_root, AnalyzeReport};
use file_engine_cli::browse::{browse_root, BrowseReport};
use file_engine_cli::decision::{apply_decisions, DecisionApplication, DecisionEntry};
use file_engine_cli::execute::{execute_decision_application, ExecuteReport};
use file_engine_cli::journal::{
    read_operation_history, recover_journal as recover_journal_file, JournalRecoveryReport,
    OperationHistoryReport,
};
use file_engine_cli::path_guard::PathGuard;
use file_engine_cli::precondition::{precheck_proposals, PrecheckReport};
use file_engine_cli::proposal::{propose_for_root, ProposalReport};
use file_engine_cli::undo::{
    undo_operation as undo_file_operation, undo_root as undo_file_root, UndoReport,
};

use crate::storage::managed_roots::{ManagedRoot, ManagedRootStore};

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn register_managed_root(
    path: String,
    store: tauri::State<'_, ManagedRootStore>,
) -> Result<ManagedRoot, String> {
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
    store: tauri::State<'_, ManagedRootStore>,
) -> Result<ExecuteReport, String> {
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
pub fn undo_last_file_operation(
    root_id: String,
    store: tauri::State<'_, ManagedRootStore>,
) -> Result<UndoReport, String> {
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
    store: tauri::State<'_, ManagedRootStore>,
) -> Result<UndoReport, String> {
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
    store: tauri::State<'_, ManagedRootStore>,
) -> Result<JournalRecoveryReport, String> {
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

    Ok(ManagedRoot {
        root_id: managed_root_id(&root.display().to_string()),
        root: root.display().to_string(),
        display_name: root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("managed root")
            .to_string(),
    })
}

#[cfg(any(feature = "tauri-commands", test))]
fn register_managed_root_in_store(
    path: String,
    store: &ManagedRootStore,
) -> Result<ManagedRoot, String> {
    let managed = register_managed_root_without_store(path)?;
    store.upsert(managed)
}

fn analyze_root_impl(root: String) -> Result<AnalyzeReport, String> {
    analyze_file_root(root).map_err(command_error)
}

fn browse_root_tree_impl(root: String, path: Option<String>) -> Result<BrowseReport, String> {
    browse_root(root, path.as_deref()).map_err(command_error)
}

fn propose_file_changes_impl(root: String) -> Result<ProposalReport, String> {
    propose_for_root(root).map_err(command_error)
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

#[cfg(test)]
mod tests {
    use std::fs;

    use file_engine_cli::decision::{Decision, DecisionEntry};
    use tempfile::tempdir;

    use crate::storage::managed_roots::ManagedRootStore;

    use super::{
        browse_root_tree, execute_file_changes, list_managed_roots, list_operation_history,
        precheck_file_changes, propose_file_changes, recover_journal, register_managed_root,
        register_managed_root_in_store, undo_operation,
    };

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

        let journal_path = root.join(".housemouse").join("journal.jsonl");
        let mut journal = fs::read_to_string(&journal_path).expect("read journal");
        journal.push_str("{not valid json\n");
        fs::write(&journal_path, journal).expect("corrupt journal");

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
    fn file_commands_reject_unknown_root_id() {
        let store = ManagedRootStore::default();

        let error = propose_file_changes("root:missing".to_string(), &store)
            .expect_err("reject unknown root id");

        assert!(error.contains("not registered"));
    }
}
