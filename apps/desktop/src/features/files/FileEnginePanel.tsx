import { useEffect, useMemo, useRef, useState } from "react";

import { ensureAgentRoom, submitCleanlinessSnapshot } from "../agent/agentApi";
import {
  analyzeRoot,
  AutoApprovalPolicy,
  autoApproveFileChanges,
  BrowseEntry,
  browseRootTree,
  calculateCleanlinessSnapshot,
  CleanlinessSnapshot,
  DecisionEntry,
  executeFileChanges,
  ExecuteReport,
  getAutoApprovalPolicy,
  IndexedFile,
  JournalCorruption,
  listenForRootChanges,
  listOperationHistory,
  listManagedRoots,
  ManagedRoot,
  OperationHistoryEntry,
  precheckFileChanges,
  PrecheckReport,
  prepareDemoRoot,
  Proposal,
  ProposalReport,
  proposeFileChanges,
  recoverJournal,
  registerManagedRoot,
  reindexManagedRoot,
  searchManagedRoot,
  selectManagedRootDirectory,
  startWatchingRoot,
  stopWatchingRoot,
  trashFile,
  updateAutoApprovalPolicy,
  updateManagedRootState,
  undoLastFileOperation,
  undoOperation,
  UndoReport
} from "./fileEngineApi";

type DecisionState = Record<string, "approved" | "rejected" | "pending">;
type RejectionReasons = Record<string, string>;
type RoomSyncState = "syncing" | "synced" | "failed";
type PrecheckSnapshot = {
  key: string;
  report: PrecheckReport;
};

const demoRootHint = "Creates a temporary copy of test-fixtures/file-trees/ui-demo";

export function FileEnginePanel({ embedded = false }: { embedded?: boolean } = {}) {
  const [pathInput, setPathInput] = useState("");
  const [roots, setRoots] = useState<ManagedRoot[]>([]);
  const [selectedRootId, setSelectedRootId] = useState("");
  const [proposal, setProposal] = useState<ProposalReport | null>(null);
  const [decisions, setDecisions] = useState<DecisionState>({});
  const [rejectionReasons, setRejectionReasons] = useState<RejectionReasons>({});
  const [history, setHistory] = useState<OperationHistoryEntry[]>([]);
  const [journalCorruption, setJournalCorruption] = useState<JournalCorruption | null>(null);
  const [browsePath, setBrowsePath] = useState("");
  const [browseEntries, setBrowseEntries] = useState<BrowseEntry[]>([]);
  const [autoApprovalPolicy, setAutoApprovalPolicy] = useState<AutoApprovalPolicy | null>(null);
  // Records which proposal_ids the "Auto approve proposals" policy pre-checked in this run, so
  // execute results can be traced back to whether a human or the policy approved them. Auto
  // approval only ever pre-fills these checkboxes here in the local manual UI; it never touches
  // delegated/mobile-originated proposals, which always execute from the server's own decision.
  const [autoApprovedProposalIds, setAutoApprovedProposalIds] = useState<Set<string>>(new Set());
  const [watchingRootIds, setWatchingRootIds] = useState<Set<string>>(new Set());
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<IndexedFile[] | null>(null);
  const [status, setStatus] = useState("Ready");
  const [error, setError] = useState<string | null>(null);
  const [resultLines, setResultLines] = useState<string[]>([]);
  const [roomSyncStates, setRoomSyncStates] = useState<Record<string, RoomSyncState>>({});
  const [precheckSnapshot, setPrecheckSnapshot] = useState<PrecheckSnapshot | null>(null);
  const [cleanlinessSnapshot, setCleanlinessSnapshot] = useState<CleanlinessSnapshot | null>(null);

  const selectedRootIdRef = useRef(selectedRootId);
  const browsePathRef = useRef(browsePath);
  const searchQueryRef = useRef(searchQuery);
  const searchResultsRef = useRef(searchResults);

  const selectedRoot = roots.find((root) => root.root_id === selectedRootId);
  const isWatchingSelectedRoot = selectedRootId ? watchingRootIds.has(selectedRootId) : false;
  const readyProposalCount = proposal?.proposals.filter((item) => item.status === "ready").length ?? 0;
  const commandDecisions = useMemo(
    () => buildDecisionEntries(decisions, rejectionReasons),
    [decisions, rejectionReasons]
  );
  const approvedCount = commandDecisions.filter((decision) => decision.decision === "approved").length;
  const proposalDecisionKey = useMemo(
    () => (proposal ? proposalSnapshotKey(selectedRootId, proposal, commandDecisions) : null),
    [selectedRootId, proposal, commandDecisions]
  );
  const activePrecheck =
    precheckSnapshot && proposalDecisionKey === precheckSnapshot.key ? precheckSnapshot.report : null;
  const activePrecheckReady =
    !!activePrecheck &&
    activePrecheck.checks.length > 0 &&
    activePrecheck.checks.every((check) => check.status === "ready");

  useEffect(() => {
    void refreshRoots();
  }, []);

  useEffect(() => {
    if (selectedRootId) {
      void refreshHistory(selectedRootId);
      void refreshAutoApprovalPolicy(selectedRootId);
    } else {
      setHistory([]);
      setAutoApprovalPolicy(null);
    }
  }, [selectedRootId]);

  useEffect(() => {
    if (selectedRootId) {
      void refreshBrowse(selectedRootId, browsePath);
    } else {
      setBrowseEntries([]);
    }
  }, [selectedRootId, browsePath]);

  useEffect(() => {
    selectedRootIdRef.current = selectedRootId;
  }, [selectedRootId]);

  useEffect(() => {
    browsePathRef.current = browsePath;
  }, [browsePath]);

  useEffect(() => {
    searchQueryRef.current = searchQuery;
  }, [searchQuery]);

  useEffect(() => {
    searchResultsRef.current = searchResults;
  }, [searchResults]);

  useEffect(() => {
    if (!window.__TAURI_INTERNALS__) return;

    let unlisten: (() => void) | undefined;
    let cancelled = false;

    listenForRootChanges((rootId) => {
      if (rootId !== selectedRootIdRef.current) return;
      setPrecheckSnapshot(null);
      void refreshBrowse(rootId, browsePathRef.current);
      void refreshHistory(rootId);
      // The watcher already refreshed the index on the Rust side; re-run any active search
      // so results reflect what just changed on disk.
      if (searchResultsRef.current !== null) {
        void runSearch(rootId, searchQueryRef.current);
      }
    }).then((stop) => {
      if (cancelled) {
        stop();
      } else {
        unlisten = stop;
      }
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  async function refreshRoots() {
    setError(null);
    try {
      const storedRoots = await listManagedRoots();
      setRoots(storedRoots);
      setSelectedRootId((current) => current || storedRoots[0]?.root_id || "");
      const results = await Promise.allSettled(
        storedRoots.map((root) => syncRootToMobile(root, false))
      );
      const failedCount = results.filter((result) => result.status === "rejected").length;
      if (failedCount > 0) {
        setStatus(`${storedRoots.length} root(s) loaded; ${failedCount} mobile sync pending`);
      } else if (storedRoots.length > 0) {
        setStatus(`${storedRoots.length} root(s) synced to mobile`);
      }
    } catch (caught) {
      setError(errorMessage(caught));
    }
  }

  async function browseForRoot() {
    setError(null);
    try {
      const selected = await selectManagedRootDirectory();
      if (selected) {
        setPathInput(selected);
      }
    } catch (caught) {
      setError(errorMessage(caught));
    }
  }

  async function prepareDemoRootPath() {
    setError(null);
    setStatus("Preparing demo root");
    try {
      const path = await prepareDemoRoot();
      setPathInput(path);
      setStatus("Demo root prepared");
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Demo setup failed");
    }
  }

  async function refreshHistory(rootId = selectedRootId) {
    if (!rootId) {
      setHistory([]);
      setJournalCorruption(null);
      return;
    }

    try {
      const report = await listOperationHistory(rootId);
      setHistory(report.operations);
      setJournalCorruption(report.corruption);
    } catch (caught) {
      setHistory([]);
      setJournalCorruption(null);
      setError(errorMessage(caught));
    }
  }

  async function refreshAutoApprovalPolicy(rootId = selectedRootId) {
    if (!rootId) {
      setAutoApprovalPolicy(null);
      return;
    }

    try {
      const policy = await getAutoApprovalPolicy(rootId);
      setAutoApprovalPolicy(policy);
    } catch (caught) {
      setAutoApprovalPolicy(null);
      setError(errorMessage(caught));
    }
  }

  async function recoverJournalForSelectedRoot() {
    if (!selectedRootId || !journalCorruption) return;

    const confirmed = window.confirm(
      "This quarantines the broken journal file and starts a fresh one. " +
        "Operations recorded before the corrupted line will no longer be undoable through the app. Continue?"
    );
    if (!confirmed) return;

    setError(null);
    setStatus("Recovering journal");
    try {
      const report = await recoverJournal(selectedRootId);
      setResultLines([`Quarantined corrupted journal to ${report.quarantined_path}`]);
      setStatus("Journal recovered");
      await refreshHistory(selectedRootId);
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Recovery failed");
    }
  }

  async function registerRoot() {
    setError(null);
    setStatus("Registering root");
    let managed: ManagedRoot;
    try {
      managed = await registerManagedRoot(pathInput.trim());
      const storedRoots = await listManagedRoots();
      setRoots(storedRoots);
      selectRoot(managed.root_id);
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Register failed");
      return;
    }
    try {
      await syncRootToMobile(managed, true);
    } catch {
      // The managed root remains registered locally and the UI exposes an explicit retry.
    }
  }

  async function syncRootToMobile(root: ManagedRoot, announce: boolean) {
    setRoomSyncStates((current) => ({ ...current, [root.root_id]: "syncing" }));
    try {
      const room = await ensureAgentRoom(root.root_id, root.display_name);
      setRoomSyncStates((current) => ({ ...current, [root.root_id]: "synced" }));
      if (announce) {
        setError(null);
        setStatus(room.created ? "Root registered and room created" : "Root synced to mobile");
      }
      return room;
    } catch (caught) {
      setRoomSyncStates((current) => ({ ...current, [root.root_id]: "failed" }));
      if (announce) {
        setError(`Root is safe locally, but mobile room sync failed: ${errorMessage(caught)}`);
        setStatus("Mobile room sync pending");
      }
      throw caught;
    }
  }

  async function updateSelectedRootState(patch: { enabled?: boolean; watch_on_startup?: boolean }) {
    if (!selectedRootId) return;

    setError(null);
    setStatus("Updating root state");
    try {
      const updated = await updateManagedRootState(selectedRootId, patch);
      setRoots((current) =>
        current.map((root) => (root.root_id === updated.root_id ? updated : root))
      );
      setStatus("Root state updated");
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Root state update failed");
    }
  }

  async function updateSelectedAutoApprovalPolicy(
    patch: Parameters<typeof updateAutoApprovalPolicy>[1]
  ) {
    if (!selectedRootId) return;

    setError(null);
    setStatus("Updating auto approval");
    try {
      const policy = await updateAutoApprovalPolicy(selectedRootId, patch);
      setAutoApprovalPolicy(policy);
      setStatus("Auto approval updated");
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Auto approval update failed");
    }
  }

  async function analyzeSelectedRoot() {
    if (!selectedRootId) return;

    setError(null);
    setStatus("Analyzing");
    try {
      const report = await analyzeRoot(selectedRootId);
      setResultLines([
        ...report.files.map((file) => `${file.path} (${file.size_bytes} bytes)`),
        ...formatSkippedEntries(report.skipped_entries)
      ]);
      setStatus(formatCountWithSkipped("Analyzed", report.files.length, report.skipped_entries.length, "files"));
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Analyze failed");
    }
  }

  async function proposeForSelectedRoot() {
    if (!selectedRootId) return;

    setError(null);
    setStatus("Building proposal");
    try {
      const report = await proposeFileChanges(selectedRootId);
      setProposal(report);
      setPrecheckSnapshot(null);
      setAutoApprovedProposalIds(new Set());
      setDecisions(
        Object.fromEntries(
          report.proposals.map((item) => [
            item.proposal_id,
            item.status === "ready" ? "approved" : "pending"
          ])
        )
      );
      setRejectionReasons({});
      setResultLines(report.proposals.map(formatProposal));
      setStatus(`Prepared ${report.proposals.length} proposals`);
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Proposal failed");
    }
  }

  async function calculateSelectedCleanliness() {
    if (!selectedRootId) return;

    setError(null);
    setStatus("Calculating cleanliness");
    try {
      const snapshot = await calculateCleanlinessSnapshot(selectedRootId);
      setCleanlinessSnapshot(snapshot);
      setResultLines(formatCleanlinessLines(snapshot));
      setStatus(`Cleanliness ${snapshot.score}/100`);
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Cleanliness calculation failed");
    }
  }

  async function syncSelectedCleanliness() {
    if (!selectedRootId) return;

    setError(null);
    setStatus("Syncing cleanliness");
    try {
      const report = await submitCleanlinessSnapshot(selectedRootId);
      setCleanlinessSnapshot(report.snapshot);
      setRoomSyncStates((current) => ({ ...current, [report.root_id]: "synced" }));
      setResultLines([
        ...formatCleanlinessLines(report.snapshot),
        `room | ${report.room_id}${report.room_created ? " (created)" : ""}`,
        `snapshot | ${report.server_snapshot.snapshot_id}`
      ]);
      setStatus(`Cleanliness synced ${report.snapshot.score}/100`);
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Cleanliness sync failed");
    }
  }

  async function precheckSelectedRoot() {
    if (!selectedRootId || !proposal) return;

    const decisionsToApply = validatedDecisionEntries();
    if (!decisionsToApply) return;

    setError(null);
    setStatus("Prechecking");
    try {
      const report = await precheckFileChanges(selectedRootId, proposal, decisionsToApply);
      if (proposalDecisionKey) {
        setPrecheckSnapshot({ key: proposalDecisionKey, report });
      }
      setResultLines(
        report.checks.map((check) =>
          [check.status, `${check.from} -> ${check.to}`, check.reason].filter(Boolean).join(" | ")
        )
      );
      const blocked = report.checks.filter((check) => check.status !== "ready");
      setStatus(
        blocked.length > 0
          ? `Precheck blocked ${blocked.length} proposal(s)`
          : `Prechecked ${report.checks.length} approved proposals`
      );
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Precheck failed");
    }
  }

  async function autoApproveSelectedProposal() {
    if (!selectedRootId || !proposal) return;

    setError(null);
    setStatus("Applying auto approval policy");
    try {
      const autoDecisions = await autoApproveFileChanges(selectedRootId, proposal);
      if (autoDecisions.length === 0) {
        setStatus("Auto approval found no eligible proposals");
        return;
      }

      setDecisions((current) => ({
        ...current,
        ...Object.fromEntries(
          autoDecisions.map((decision) => [decision.proposal_id, decision.decision])
        )
      }));
      // Record which items the policy pre-approved, distinct from ones a human checked. This
      // does not skip precheck or execute: the checkboxes above still drive the same
      // precheck-then-execute gate as a manual approval.
      const autoApprovedIds = new Set(autoDecisions.map((decision) => decision.proposal_id));
      setAutoApprovedProposalIds(autoApprovedIds);
      const autoApprovedItems = proposal.proposals.filter((item) =>
        autoApprovedIds.has(item.proposal_id)
      );
      setResultLines([
        `Auto approved ${autoDecisions.length} proposal(s) — still requires precheck and execute:`,
        ...autoApprovedItems.map((item) => `auto-approved | ${item.from} -> ${item.to}`)
      ]);
      setStatus(`Auto approved ${autoDecisions.length} proposals`);
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Auto approval failed");
    }
  }

  async function executeSelectedRoot() {
    if (!selectedRootId || !proposal) return;

    const decisionsToApply = validatedDecisionEntries();
    if (!decisionsToApply) return;
    if (!proposalDecisionKey || !activePrecheck) {
      setError("Run precheck after the latest proposal or decision change before executing.");
      setStatus("Precheck required");
      return;
    }
    if (!activePrecheckReady) {
      setResultLines(formatPrecheckLines(activePrecheck));
      setStatus("Execute blocked by precheck");
      return;
    }

    setError(null);
    setStatus("Ready to execute");
    try {
      const confirmed = window.confirm(
        `Move ${activePrecheck.checks.length} approved files for ${selectedRoot?.display_name || "this root"}?`
      );
      if (!confirmed) {
        setStatus("Execute cancelled");
        return;
      }

      setStatus("Executing");
      const autoApprovedCount = decisionsToApply.filter(
        (decision) =>
          decision.decision === "approved" && autoApprovedProposalIds.has(decision.proposal_id)
      ).length;
      const report = await executeFileChanges(selectedRootId, proposal, decisionsToApply);
      const lines = formatExecuteLines(report);
      setResultLines(lines);
      setPrecheckSnapshot(null);
      setAutoApprovedProposalIds(new Set());
      setStatus(
        `Executed ${report.executed_count}, skipped ${report.skipped_count}, rejected ${report.rejected_count}` +
          (autoApprovedCount > 0 ? ` (${autoApprovedCount} auto-approved)` : "")
      );
      await refreshHistory(selectedRootId);
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Execute failed");
    }
  }

  async function undoSelectedRoot() {
    if (!selectedRootId) return;

    const confirmed = window.confirm(`Undo latest file operation for ${selectedRoot?.display_name || "this root"}?`);
    if (!confirmed) return;

    setError(null);
    setStatus("Undoing");
    try {
      const report = await undoLastFileOperation(selectedRootId);
      const lines = formatUndoLines(report);
      setResultLines(lines);
      setStatus(`Undone ${report.undone_count}, skipped ${report.skipped_count}`);
      await refreshHistory(selectedRootId);
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Undo failed");
    }
  }

  async function undoSelectedOperation(operation: OperationHistoryEntry) {
    if (!selectedRootId || !operation.can_undo) return;

    const confirmed = window.confirm(`Undo ${operation.from} -> ${operation.to}?`);
    if (!confirmed) return;

    setError(null);
    setStatus("Undoing selected operation");
    try {
      const report = await undoOperation(selectedRootId, operation.operation_id);
      const lines = formatUndoLines(report);
      setResultLines(lines);
      setStatus(`Undone ${report.undone_count}, skipped ${report.skipped_count}`);
      await refreshHistory(selectedRootId);
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Undo failed");
    }
  }

  async function toggleWatchSelectedRoot() {
    if (!selectedRootId) return;

    setError(null);
    try {
      if (isWatchingSelectedRoot) {
        await stopWatchingRoot(selectedRootId);
        setWatchingRootIds((current) => {
          const next = new Set(current);
          next.delete(selectedRootId);
          return next;
        });
        setStatus("Stopped watching for changes");
      } else {
        await startWatchingRoot(selectedRootId);
        setWatchingRootIds((current) => new Set(current).add(selectedRootId));
        setStatus("Watching for changes");
      }
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Watch toggle failed");
    }
  }

  async function refreshBrowse(rootId = selectedRootId, path = browsePath) {
    if (!rootId) {
      setBrowseEntries([]);
      return;
    }

    try {
      const report = await browseRootTree(rootId, path);
      setBrowseEntries(report.entries);
      if (report.skipped_entries.length > 0) {
        setStatus(`Browsing ${report.path || "/"} (${report.skipped_entries.length} skipped)`);
      }
    } catch (caught) {
      setBrowseEntries([]);
      setError(errorMessage(caught));
    }
  }

  async function runSearch(rootId = selectedRootId, query = searchQuery) {
    if (!rootId) return;

    setError(null);
    try {
      const report = await searchManagedRoot(rootId, query);
      setSearchResults(report.files);
      setStatus(formatCountWithSkipped("Found", report.files.length, report.skipped_entries.length, "indexed files"));
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Search failed");
    }
  }

  async function reindexSelectedRoot() {
    if (!selectedRootId) return;

    setError(null);
    setStatus("Reindexing");
    try {
      const report = await reindexManagedRoot(selectedRootId);
      setStatus(formatCountWithSkipped("Indexed", report.files.length, report.skipped_entries.length, "files"));
      // Refresh whatever the search box is currently showing against the new index.
      if (searchResults !== null) {
        await runSearch(selectedRootId, searchQuery);
      }
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Reindex failed");
    }
  }

  async function trashBrowseEntry(entry: BrowseEntry) {
    if (!selectedRootId || entry.is_dir) return;

    const confirmed = window.confirm(`Move ${entry.path} into recoverable trash?`);
    if (!confirmed) return;

    setError(null);
    setStatus("Moving file to trash");
    try {
      setPrecheckSnapshot(null);
      const report = await trashFile(selectedRootId, entry.path);
      setResultLines([
        `trashed | ${report.original_path} -> ${report.trashed_path}`,
        `metadata | ${report.metadata_path}`,
        `operation | ${report.operation_id}`
      ]);
      setStatus("File moved to recoverable trash");
      await refreshBrowse(selectedRootId, browsePath);
      await refreshHistory(selectedRootId);
      if (searchResults !== null) {
        await runSearch(selectedRootId, searchQuery);
      }
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Trash failed");
    }
  }

  function openBrowseEntry(entry: BrowseEntry) {
    if (entry.is_dir) {
      setBrowsePath(entry.path);
    }
  }

  function openBrowseSegment(index: number) {
    const segments = browsePath.split("/").filter(Boolean);
    setBrowsePath(segments.slice(0, index + 1).join("/"));
  }

  function selectRoot(rootId: string) {
    setSelectedRootId(rootId);
    setProposal(null);
    setPrecheckSnapshot(null);
    setAutoApprovedProposalIds(new Set());
    setDecisions({});
    setRejectionReasons({});
    setResultLines([]);
    setBrowsePath("");
    setSearchQuery("");
    setSearchResults(null);
    setCleanlinessSnapshot(null);
  }

  function setDecision(item: Proposal, decision: DecisionState[string]) {
    setDecisions((current) => ({ ...current, [item.proposal_id]: decision }));
  }

  function setRejectionReason(item: Proposal, reason: string) {
    setRejectionReasons((current) => ({ ...current, [item.proposal_id]: reason }));
  }

  function setAutoApprovalAction(action: "move" | "trash", enabled: boolean) {
    const current = autoApprovalPolicy?.allowed_actions || [];
    const next = enabled
      ? Array.from(new Set([...current, action]))
      : current.filter((item) => item !== action);
    if (next.length === 0) {
      setError("Auto approval must keep at least one allowed action.");
      return;
    }

    void updateSelectedAutoApprovalPolicy({ allowed_actions: next });
  }

  function validatedDecisionEntries() {
    const missingReason = proposal?.proposals.find(
      (item) => decisions[item.proposal_id] === "rejected" && !rejectionReasons[item.proposal_id]?.trim()
    );

    if (missingReason) {
      setError(`Rejected proposal needs a reason: ${missingReason.from}`);
      setStatus("Decision invalid");
      return null;
    }

    if (commandDecisions.length === 0) {
      setError("Approve or reject at least one proposal before continuing.");
      setStatus("No decisions selected");
      return null;
    }

    return commandDecisions;
  }

  return (
    <div className={embedded ? undefined : "app-shell"}>
      <section className="toolbar">
        <div>
          <h1>MouseKeeper Files</h1>
          <p>{status}</p>
        </div>
        <button type="button" onClick={refreshRoots}>
          Refresh
        </button>
      </section>

      <section className="panel">
        <label htmlFor="root-path">Managed root path</label>
        <div className="input-row">
          <input
            id="root-path"
            value={pathInput}
            onChange={(event) => setPathInput(event.target.value)}
            placeholder={demoRootHint}
          />
          <button type="button" onClick={browseForRoot}>
            Browse
          </button>
          <button type="button" onClick={() => void prepareDemoRootPath()}>
            Demo
          </button>
          <button type="button" onClick={registerRoot} disabled={!pathInput.trim()}>
            Register
          </button>
        </div>
        <p className="path-text">
          Register is enabled after a path is entered. Tauri commands run inside the desktop app,
          not a normal browser tab.
        </p>
      </section>

      <section className="workspace-grid">
        <div className="panel">
          <label htmlFor="root-select">Registered roots</label>
          <select
            id="root-select"
            value={selectedRootId}
            onChange={(event) => selectRoot(event.target.value)}
          >
            <option value="">No root selected</option>
            {roots.map((root) => (
              <option key={root.root_id} value={root.root_id}>
                {root.display_name}
              </option>
            ))}
          </select>
          {selectedRoot ? <p className="path-text">{selectedRoot.root}</p> : null}
          {selectedRoot ? (
            <div className="root-state-grid">
              <label>
                <input
                  type="checkbox"
                  checked={selectedRoot.enabled}
                  onChange={(event) =>
                    void updateSelectedRootState({ enabled: event.target.checked })
                  }
                />
                Enabled
              </label>
              <label>
                <input
                  type="checkbox"
                  checked={selectedRoot.watch_on_startup}
                  onChange={(event) =>
                    void updateSelectedRootState({ watch_on_startup: event.target.checked })
                  }
                />
                Watch on startup
              </label>
              <small>{selectedRoot.last_seen_status}</small>
            </div>
          ) : null}
          {selectedRoot ? (
            <div className="agent-actions">
              <span className="path-text">
                Mobile room: {roomSyncStates[selectedRoot.root_id] ?? "pending"}
              </span>
              <button
                type="button"
                disabled={roomSyncStates[selectedRoot.root_id] === "syncing"}
                onClick={() => void syncRootToMobile(selectedRoot, true)}
              >
                Sync to mobile
              </button>
            </div>
          ) : null}
          {selectedRoot && autoApprovalPolicy ? (
            <div className="auto-approval-panel">
              <label>
                <input
                  type="checkbox"
                  checked={autoApprovalPolicy.enabled}
                  onChange={(event) =>
                    void updateSelectedAutoApprovalPolicy({ enabled: event.target.checked })
                  }
                />
                Auto approve proposals
              </label>
              <label>
                <input
                  type="checkbox"
                  checked={autoApprovalPolicy.allowed_actions.includes("trash")}
                  onChange={(event) => setAutoApprovalAction("trash", event.target.checked)}
                />
                Trash
              </label>
              <label>
                <input
                  type="checkbox"
                  checked={autoApprovalPolicy.allowed_actions.includes("move")}
                  onChange={(event) => setAutoApprovalAction("move", event.target.checked)}
                />
                Move
              </label>
              <label>
                Max
                <input
                  type="number"
                  min="1"
                  value={autoApprovalPolicy.max_files_per_run}
                  onChange={(event) =>
                    void updateSelectedAutoApprovalPolicy({
                      max_files_per_run: Math.max(1, Number(event.target.value) || 1)
                    })
                  }
                />
              </label>
              <small className="auto-approval-hint">
                Pre-checks ready proposal items for the actions above. It does not run file
                operations by itself — precheck and execute below are still required, and this
                policy never applies to proposals started from mobile or the agent.
              </small>
            </div>
          ) : null}

          <div className="button-grid">
            <button type="button" onClick={analyzeSelectedRoot} disabled={!selectedRootId}>
              Analyze
            </button>
            <button type="button" onClick={proposeForSelectedRoot} disabled={!selectedRootId}>
              Build proposal
            </button>
            <button
              type="button"
              onClick={() => void calculateSelectedCleanliness()}
              disabled={!selectedRootId}
            >
              Cleanliness
            </button>
            <button
              type="button"
              onClick={() => void syncSelectedCleanliness()}
              disabled={!selectedRootId}
            >
              Sync cleanliness
            </button>
            <button
              type="button"
              onClick={precheckSelectedRoot}
              disabled={!selectedRootId || !proposal}
            >
              Precheck
            </button>
            <button
              type="button"
              onClick={() => void autoApproveSelectedProposal()}
              disabled={!selectedRootId || !proposal || !autoApprovalPolicy?.enabled}
              title="Pre-checks ready proposal items only. Precheck and execute are still required."
            >
              Auto approve proposals
            </button>
            <button
              type="button"
              onClick={executeSelectedRoot}
              disabled={
                !selectedRootId ||
                !proposal ||
                approvedCount === 0 ||
                !activePrecheckReady ||
                !!journalCorruption
              }
            >
              Execute
            </button>
            <button
              type="button"
              onClick={undoSelectedRoot}
              disabled={!selectedRootId || !!journalCorruption}
            >
              Undo
            </button>
            <button
              type="button"
              onClick={() => void toggleWatchSelectedRoot()}
              disabled={!selectedRootId}
            >
              {isWatchingSelectedRoot ? "Stop watching" : "Watch for changes"}
            </button>
          </div>
          {proposal ? (
            <p className="path-text">
              {readyProposalCount} ready, {approvedCount} approved, precheck{" "}
              {activePrecheckReady ? "ready" : activePrecheck ? "blocked" : "required"}
            </p>
          ) : null}
          {cleanlinessSnapshot ? (
            <p className="path-text">
              Cleanliness {cleanlinessSnapshot.score}/100 |{" "}
              {cleanlinessSnapshot.metrics.unorganizedFileCount} unorganized of{" "}
              {cleanlinessSnapshot.metrics.totalFileCount}
            </p>
          ) : null}
          <p className="mode-note">
            Delegated cleanup uses proposals only. Mobile, agent, and AI requests cannot run the
            manual file tools directly.
          </p>
          {journalCorruption ? (
            <p className="error-text">Journal needs recovery before execute/undo will run.</p>
          ) : null}
          {isWatchingSelectedRoot ? (
            <p className="path-text">Watching for changes — Browse and History refresh automatically.</p>
          ) : null}
        </div>

        <div className="panel">
          <h2>Delegated proposals</h2>
          <div className="proposal-list">
            {proposal?.proposals.map((item) => (
              <article key={item.proposal_id} className="proposal-row">
                <div>
                  <strong>{item.from}</strong>
                  <span>{item.to}</span>
                  <small>{item.status}</small>
                  <small>decision: {decisions[item.proposal_id] || "pending"}</small>
                  {activePrecheck ? (
                    <small>{formatPrecheckSummary(activePrecheck, item)}</small>
                  ) : null}
                  {decisions[item.proposal_id] === "rejected" ? (
                    <input
                      value={rejectionReasons[item.proposal_id] || ""}
                      onChange={(event) => setRejectionReason(item, event.target.value)}
                      placeholder="Reason for rejection"
                    />
                  ) : null}
                </div>
                <select
                  value={decisions[item.proposal_id] || "pending"}
                  onChange={(event) =>
                    setDecision(item, event.target.value as DecisionState[string])
                  }
                >
                  <option value="approved" disabled={item.status !== "ready"}>
                    Approve
                  </option>
                  <option value="pending">Skip</option>
                  <option value="rejected">Reject</option>
                </select>
              </article>
            )) || <p>No proposal loaded.</p>}
          </div>
        </div>
      </section>

      <section className="panel">
        <div className="section-header">
          <h2>Search</h2>
          <button type="button" onClick={reindexSelectedRoot} disabled={!selectedRootId}>
            Reindex
          </button>
        </div>
        <form
          className="input-row search-row"
          onSubmit={(event) => {
            event.preventDefault();
            void runSearch();
          }}
        >
          <input
            value={searchQuery}
            onChange={(event) => setSearchQuery(event.target.value)}
            placeholder="Search indexed files by name"
            disabled={!selectedRootId}
          />
          <button type="submit" disabled={!selectedRootId}>
            Search
          </button>
        </form>
        <div className="search-list">
          {searchResults?.map((file) => (
            <article key={file.relative_path} className="search-row-item">
              <span>📄</span>
              <strong>{file.relative_path}</strong>
              <small>{formatBrowseSize(file.size_bytes)}</small>
            </article>
          ))}
          {searchResults?.length === 0 ? <p>No indexed files match.</p> : null}
          {searchResults === null && selectedRootId ? (
            <p>Reindex, then search the managed root's files. Watching a root keeps its index fresh.</p>
          ) : null}
          {!selectedRootId ? <p>Select a root to search its files.</p> : null}
        </div>
      </section>

      <section className="panel">
        <div className="section-header">
          <div>
            <h2>Manual file tools</h2>
            <p className="path-text">
              These buttons are local-only actions started by this desktop user.
            </p>
          </div>
          <button type="button" onClick={() => void refreshBrowse()} disabled={!selectedRootId}>
            Refresh
          </button>
        </div>
        <nav className="breadcrumb">
          <button type="button" onClick={() => setBrowsePath("")} disabled={!selectedRootId}>
            {selectedRoot?.display_name || "root"}
          </button>
          {browsePath
            .split("/")
            .filter(Boolean)
            .map((segment, index) => (
              <span key={`${segment}-${index}`}>
                {" / "}
                <button type="button" onClick={() => openBrowseSegment(index)}>
                  {segment}
                </button>
              </span>
            ))}
        </nav>
        <div className="browse-list">
          {browseEntries.map((entry) => (
            <article key={entry.path} className="browse-row">
              <span>{entry.is_dir ? "📁" : "📄"}</span>
              {entry.is_dir ? (
                <button
                  type="button"
                  className="link-button"
                  onClick={() => openBrowseEntry(entry)}
                >
                  {entry.name}
                </button>
              ) : (
                <strong>{entry.name}</strong>
              )}
              {!entry.is_dir ? <small>{formatBrowseSize(entry.size_bytes)}</small> : null}
              {!entry.is_dir ? (
                <button
                  type="button"
                  className="danger-button"
                  onClick={() => void trashBrowseEntry(entry)}
                  disabled={!!journalCorruption}
                >
                  Trash
                </button>
              ) : null}
            </article>
          ))}
          {selectedRootId && browseEntries.length === 0 ? <p>This folder is empty.</p> : null}
          {!selectedRootId ? <p>Select a root to browse its files.</p> : null}
        </div>
      </section>

      <section className="panel">
        <h2>Output</h2>
        {error ? <p className="error-text">{error}</p> : null}
        <pre>{resultLines.join("\n") || "No output yet."}</pre>
      </section>

      <section className="panel">
        <div className="section-header">
          <h2>History</h2>
          <button type="button" onClick={() => void refreshHistory()} disabled={!selectedRootId}>
            Refresh
          </button>
        </div>
        {journalCorruption ? (
          <div className="recovery-banner">
            <p>
              <strong>Recovery needed.</strong> Journal is unreadable starting at line{" "}
              {journalCorruption.line}: {journalCorruption.message}
            </p>
            <p>
              History above this point is still shown, but new executes and undos are blocked
              until the broken journal is quarantined. Operations recorded before the break will
              no longer be undoable through the app after recovery.
            </p>
            <button type="button" onClick={() => void recoverJournalForSelectedRoot()}>
              Recover journal
            </button>
          </div>
        ) : null}
        <div className="history-list">
          {history.map((operation) => (
            <article key={operation.operation_id} className="history-row">
              <strong>{operation.latest_status}</strong>
              <span>{`${operation.from} -> ${operation.to}`}</span>
              <small>{new Date(operation.created_unix_ms).toLocaleString()}</small>
              {!operation.can_undo && operation.undo_blocked_reason ? (
                <small className="reason-text">Can't undo: {operation.undo_blocked_reason}</small>
              ) : null}
              <button
                type="button"
                onClick={() => void undoSelectedOperation(operation)}
                disabled={!operation.can_undo || !!journalCorruption}
              >
                Undo
              </button>
            </article>
          ))}
          {history.length === 0 ? <p>No journal history yet.</p> : null}
        </div>
      </section>
    </div>
  );
}

function buildDecisionEntries(decisions: DecisionState, rejectionReasons: RejectionReasons) {
  return Object.entries(decisions)
    .filter(([, decision]) => decision !== "pending")
    .map(([proposal_id, decision]): DecisionEntry => {
      if (decision === "approved") {
        return { proposal_id, decision };
      }

      if (decision === "rejected") {
        return {
          proposal_id,
          decision,
          reason: rejectionReasons[proposal_id]?.trim()
        };
      }

      throw new Error(`Unsupported decision: ${decision}`);
    });
}

function formatBrowseSize(sizeBytes: number | null) {
  if (sizeBytes === null) return "";
  if (sizeBytes < 1024) return `${sizeBytes} B`;
  return `${(sizeBytes / 1024).toFixed(1)} KB`;
}

function formatProposal(item: Proposal) {
  return `${item.status} | ${item.from} -> ${item.to} | ${item.reason}`;
}

function formatCleanlinessLines(snapshot: CleanlinessSnapshot) {
  return [
    `score | ${snapshot.score}/100`,
    `files | total ${snapshot.metrics.totalFileCount}, managed ${snapshot.metrics.managedFileCount}, unorganized ${snapshot.metrics.unorganizedFileCount}`,
    `calculated | ${snapshot.calculatedAt}`,
    ...snapshot.metrics.deductions.map(
      (deduction) =>
        `deduction | ${deduction.reasonCode} | count ${deduction.count} | -${deduction.points}`
    )
  ];
}

function formatExecuteLines(report: ExecuteReport) {
  return ["executed", "skipped", "rejected"].flatMap((status) => {
    const rows = report.results.filter((result) => result.status === status);
    if (rows.length === 0) return [];
    return [
      `[${status}]`,
      ...rows.map((result) =>
        [result.status, `${result.from} -> ${result.to}`, result.reason].filter(Boolean).join(" | ")
      )
    ];
  });
}

function formatUndoLines(report: UndoReport) {
  return report.results.map((result) =>
    [result.status, `${result.from} -> ${result.to}`, result.reason].filter(Boolean).join(" | ")
  );
}

function formatPrecheckLines(report: PrecheckReport) {
  return report.checks.map((check) =>
    [check.status, `${check.from} -> ${check.to}`, check.reason].filter(Boolean).join(" | ")
  );
}

function formatPrecheckSummary(report: PrecheckReport, item: Proposal) {
  const check = report.checks.find((entry) => entry.from === item.from && entry.to === item.to);
  return check ? `precheck: ${check.status}` : "precheck: not selected";
}

function proposalSnapshotKey(
  rootId: string,
  proposal: ProposalReport,
  decisions: DecisionEntry[]
) {
  return JSON.stringify({ rootId, proposal, decisions });
}

function formatSkippedEntries(entries: Array<{ path: string; reason: string }>) {
  return entries.map((entry) => `skipped | ${entry.path || "/"} | ${entry.reason}`);
}

function formatCountWithSkipped(label: string, count: number, skipped: number, noun: string) {
  return skipped > 0 ? `${label} ${count} ${noun}, skipped ${skipped}` : `${label} ${count} ${noun}`;
}

function errorMessage(caught: unknown) {
  return caught instanceof Error ? caught.message : String(caught);
}
