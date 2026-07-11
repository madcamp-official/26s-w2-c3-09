import { invoke } from "@tauri-apps/api/core";
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
};

export type FileEntry = {
  path: string;
  size_bytes: number;
  modified_unix_ms: number | null;
};

export type AnalyzeReport = {
  root: string;
  files: FileEntry[];
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
};

export type ProposalStatus = "ready" | "destination_exists";

export type Proposal = {
  proposal_id: string;
  action: "move";
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
  action: "move";
  from: string;
  to: string;
  latest_status: "planned" | "executed" | "undo_planned" | "undone";
  created_unix_ms: number;
  can_undo: boolean;
  undo_blocked_reason: string | null;
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

export function analyzeRoot(rootId: string) {
  return invokeCommand<AnalyzeReport>("analyze_root", { rootId });
}

export function browseRootTree(rootId: string, path?: string) {
  return invokeCommand<BrowseReport>("browse_root_tree", { rootId, path: path || null });
}

export function proposeFileChanges(rootId: string) {
  return invokeCommand<ProposalReport>("propose_file_changes", { rootId });
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
