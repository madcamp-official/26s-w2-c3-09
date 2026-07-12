import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export type ManagedRoot = {
  root_id: string;
  root: string;
  display_name: string;
  enabled: boolean;
  watch_on_startup: boolean;
  last_seen_status: "ready" | "missing" | "error";
  last_error: string | null;
  registered_unix_ms: number;
  updated_unix_ms: number;
};

export type ManagedRootStatePatch = {
  enabled?: boolean;
  watch_on_startup?: boolean;
};

export type FileEntry = {
  path: string;
  size_bytes: number;
  modified_unix_ms: number | null;
};

export type SkippedEntry = {
  path: string;
  reason: string;
};

export type AnalyzeReport = {
  root: string;
  files: FileEntry[];
  skipped_entries: SkippedEntry[];
};

export type BrowseEntry = {
  name: string;
  path: string;
  is_dir: boolean;
  size_bytes: number | null;
  modified_unix_ms: number | null;
};

export type BrowseReport = {
  root: string;
  path: string;
  entries: BrowseEntry[];
  skipped_entries: SkippedEntry[];
};

export type IndexedFile = {
  relative_path: string;
  size_bytes: number;
  modified_unix_ms: number | null;
  extension: string | null;
};

export type FileIndexReport = {
  root: string;
  files: IndexedFile[];
  skipped_entries: SkippedEntry[];
};

export type ProposalStatus = "ready" | "destination_exists";
export type ProposalAction = "move" | "trash";

export type Proposal = {
  proposal_id: string;
  action: ProposalAction;
  from: string;
  to: string;
  source_size_bytes: number;
  source_modified_unix_ms: number | null;
  reason: string;
  status: ProposalStatus;
};

export type ProposalReport = {
  root: string;
  proposals: Proposal[];
};

export type AutoApprovalPolicy = {
  root_id: string;
  enabled: boolean;
  allowed_actions: ProposalAction[];
  max_files_per_run: number;
  expires_unix_ms: number | null;
  updated_unix_ms: number;
};

export type AutoApprovalPolicyPatch = {
  enabled?: boolean;
  allowed_actions?: ProposalAction[];
  max_files_per_run?: number;
  expires_unix_ms?: number;
};

export type DecisionEntry = {
  proposal_id: string;
  decision: "approved" | "rejected";
  reason?: string;
};

export type PrecheckStatus =
  | "ready"
  | "destination_exists"
  | "missing_source"
  | "source_changed"
  | "rejected_path";

export type PrecheckResult = {
  action: "move" | "trash";
  from: string;
  to: string;
  status: PrecheckStatus;
  reason: string | null;
};

export type PrecheckReport = {
  root: string;
  checks: PrecheckResult[];
};

export type ExecuteStatus = "executed" | "skipped" | "rejected";

export type ExecuteResult = {
  action: "move" | "trash";
  from: string;
  to: string;
  status: ExecuteStatus;
  reason: string | null;
};

export type ExecuteReport = {
  root: string;
  journal_path: string;
  executed_count: number;
  skipped_count: number;
  rejected_count: number;
  results: ExecuteResult[];
};

export type UndoReport = {
  root: string;
  journal_path: string;
  undone_count: number;
  skipped_count: number;
  results: Array<{
    from: string;
    to: string;
    status: "undone" | "skipped";
    reason: string | null;
  }>;
};

export type OperationHistoryEntry = {
  operation_id: string;
  action: "move" | "trash";
  from: string;
  to: string;
  latest_status: "planned" | "executed" | "undo_planned" | "undone";
  created_unix_ms: number;
  can_undo: boolean;
  undo_blocked_reason: string | null;
};

export type TrashReport = {
  root: string;
  journal_path: string;
  operation_id: string;
  original_path: string;
  trashed_path: string;
  metadata_path: string;
};

export type CreateFileReport = {
  root: string;
  created_path: string;
};

export type RenameFileReport = {
  root: string;
  journal_path: string;
  operation_id: string;
  from: string;
  to: string;
};

export type JournalCorruption = {
  line: number;
  message: string;
};

export type OperationHistoryReport = {
  root: string;
  journal_path: string;
  operations: OperationHistoryEntry[];
  corruption: JournalCorruption | null;
};

export type JournalRecoveryReport = {
  root: string;
  journal_path: string;
  quarantined_path: string;
};

export function registerManagedRoot(path: string) {
  return invokeCommand<ManagedRoot>("register_managed_root", { path });
}

export function listManagedRoots() {
  return invokeCommand<ManagedRoot[]>("list_managed_roots");
}

export function updateManagedRootState(rootId: string, patch: ManagedRootStatePatch) {
  return invokeCommand<ManagedRoot>("update_managed_root_state", { rootId, patch });
}

export function prepareDemoRoot() {
  return invokeCommand<string>("prepare_demo_root");
}

export function analyzeRoot(rootId: string) {
  return invokeCommand<AnalyzeReport>("analyze_root", { rootId });
}

export function browseRootTree(rootId: string, path?: string) {
  return invokeCommand<BrowseReport>("browse_root_tree", { rootId, path: path || null });
}

export function reindexManagedRoot(rootId: string) {
  return invokeCommand<FileIndexReport>("reindex_managed_root", { rootId });
}

export function searchManagedRoot(rootId: string, query: string) {
  return invokeCommand<FileIndexReport>("search_managed_root", { rootId, query });
}

export function proposeFileChanges(rootId: string) {
  return invokeCommand<ProposalReport>("propose_file_changes", { rootId });
}

/**
 * Locally validates an AI-produced Rule DSL draft (plan item 12) without touching the filesystem.
 * Resolves when the draft is a well-formed, safe rule set; rejects otherwise. Use this to reject
 * bad AI output before it enters the command/proposal pipeline. Validation only — never mutates.
 */
export function validateRuleDraft(draft: unknown) {
  return invokeCommand<void>("validate_rule_draft", { draft });
}

export function getAutoApprovalPolicy(rootId: string) {
  return invokeCommand<AutoApprovalPolicy>("get_auto_approval_policy", { rootId });
}

export function updateAutoApprovalPolicy(rootId: string, patch: AutoApprovalPolicyPatch) {
  return invokeCommand<AutoApprovalPolicy>("update_auto_approval_policy", { rootId, patch });
}

export function autoApproveFileChanges(rootId: string, proposal: ProposalReport) {
  return invokeCommand<DecisionEntry[]>("auto_approve_file_changes", { rootId, proposal });
}

export function precheckFileChanges(
  rootId: string,
  proposal: ProposalReport,
  decisions: DecisionEntry[]
) {
  return invokeCommand<PrecheckReport>("precheck_file_changes", {
    rootId,
    proposal,
    decisions
  });
}

export function executeFileChanges(
  rootId: string,
  proposal: ProposalReport,
  decisions: DecisionEntry[]
) {
  return invokeCommand<ExecuteReport>("execute_file_changes", {
    rootId,
    proposal,
    decisions
  });
}

export function trashFile(rootId: string, path: string) {
  return invokeCommand<TrashReport>("trash_file", { rootId, path });
}

export function createFile(rootId: string, path: string) {
  return invokeCommand<CreateFileReport>("create_file", { rootId, path });
}

export function renameFile(rootId: string, path: string, newName: string) {
  return invokeCommand<RenameFileReport>("rename_file", { rootId, path, newName });
}

export function undoLastFileOperation(rootId: string) {
  return invokeCommand<UndoReport>("undo_last_file_operation", { rootId });
}

export function undoOperation(rootId: string, operationId: string) {
  return invokeCommand<UndoReport>("undo_operation", { rootId, operationId });
}

export function listOperationHistory(rootId: string) {
  return invokeCommand<OperationHistoryReport>("list_operation_history", { rootId });
}

export function recoverJournal(rootId: string) {
  return invokeCommand<JournalRecoveryReport>("recover_journal", { rootId });
}

export const ROOT_CHANGED_EVENT = "managed-root-changed";

export function startWatchingRoot(rootId: string) {
  return invokeCommand<void>("start_watching_root", { rootId });
}

export function stopWatchingRoot(rootId: string) {
  return invokeCommand<boolean>("stop_watching_root", { rootId });
}

export function isWatchingRoot(rootId: string) {
  return invokeCommand<boolean>("is_watching_root", { rootId });
}

export function listenForRootChanges(handler: (rootId: string) => void) {
  ensureTauriRuntime();

  return listen<string>(ROOT_CHANGED_EVENT, (event) => handler(event.payload));
}

export async function selectManagedRootDirectory() {
  ensureTauriRuntime();

  const selected = await open({
    directory: true,
    multiple: false,
    title: "Select a managed root"
  });

  return typeof selected === "string" ? selected : null;
}

function invokeCommand<T>(command: string, args?: Record<string, unknown>) {
  ensureTauriRuntime();

  return invoke<T>(command, args);
}

function ensureTauriRuntime() {
  if (!window.__TAURI_INTERNALS__) {
    throw new Error(
      "Tauri runtime is not available. Run the desktop app with `cargo run --features tauri-commands` from apps/desktop/src-tauri, not only the Vite browser page."
    );
  }
}
