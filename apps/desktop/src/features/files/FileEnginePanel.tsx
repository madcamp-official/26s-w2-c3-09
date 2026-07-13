import { useEffect, useMemo, useRef, useState } from "react";

import {
  disconnectAgentRoom,
  ensureAgentRoom,
  preflightAgentRoomDisconnect,
  submitCleanlinessSnapshot
} from "../agent/agentApi";
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
  getLatestCleanlinessSnapshot,
  IndexedFile,
  JournalCorruption,
  listenForCleanlinessSnapshotUpdates,
  listenForManagedRootBindingChanges,
  listenForAutoCleanupProposals,
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
  unregisterManagedRoot,
  updateAutoApprovalPolicy,
  updateManagedRootState,
  undoLastFileOperation,
  undoOperation,
  UndoReport
} from "./fileEngineApi";

type DecisionState = Record<string, "approved" | "rejected" | "pending">;
type RejectionReasons = Record<string, string>;
type RoomSyncState = "syncing" | "synced" | "failed" | "detached" | "unbound";
type PrecheckSnapshot = {
  key: string;
  report: PrecheckReport;
};

const demoRootHint = "test-fixtures/file-trees/ui-demo의 임시 복사본을 만들어요";

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
  const [status, setStatus] = useState("준비됨");
  const [error, setError] = useState<string | null>(null);
  const [resultLines, setResultLines] = useState<string[]>([]);
  const [roomSyncStates, setRoomSyncStates] = useState<Record<string, RoomSyncState>>({});
  const [precheckSnapshot, setPrecheckSnapshot] = useState<PrecheckSnapshot | null>(null);
  const [cleanlinessSnapshot, setCleanlinessSnapshot] = useState<CleanlinessSnapshot | null>(null);

  const selectedRootIdRef = useRef(selectedRootId);
  const proposalRef = useRef(proposal);
  const browsePathRef = useRef(browsePath);
  const searchQueryRef = useRef(searchQuery);
  const searchResultsRef = useRef(searchResults);
  const roomDisconnectKeys = useRef<Record<string, string>>({});
  const roomDisconnectAcknowledgements = useRef<Set<string>>(new Set());
  const rootUnregisterKeys = useRef<Record<string, string>>({});

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
    if (!window.__TAURI_INTERNALS__) return;

    let unlisten: (() => void) | undefined;
    let cancelled = false;
    listenForManagedRootBindingChanges(() => {
      void reloadDurableRootBindings();
    }).then((stop) => {
      if (cancelled) stop();
      else unlisten = stop;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (selectedRootId) {
      void refreshHistory(selectedRootId);
      void refreshAutoApprovalPolicy(selectedRootId);
      void refreshLatestCleanliness(selectedRootId);
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
    proposalRef.current = proposal;
  }, [proposal]);

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

    let unlistenRootChanges: (() => void) | undefined;
    let unlistenAutoProposals: (() => void) | undefined;
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
        unlistenRootChanges = stop;
      }
    });

    listenForAutoCleanupProposals((event) => {
      if (event.root_id !== selectedRootIdRef.current) return;
      // Never replace a proposal while the user is choosing decisions or has completed precheck.
      // The next background tick can offer fresh work after execute/selectRoot clears this ref.
      if (proposalRef.current) return;

      const autoApprovedIds = new Set(event.auto_approved_proposal_ids);
      proposalRef.current = event.proposal;
      setProposal(event.proposal);
      setPrecheckSnapshot(null);
      setAutoApprovedProposalIds(autoApprovedIds);
      setDecisions(
        Object.fromEntries(
          event.proposal.proposals.map((item) => [
            item.proposal_id,
            autoApprovedIds.has(item.proposal_id) ? "approved" : "pending"
          ])
        )
      );
      setRejectionReasons({});
      setResultLines([
        ...event.proposal.proposals.map(formatProposal),
        `자동 제안 ${event.proposal.proposals.length}건 준비됨 · 정책 사전 승인 ${event.auto_approved_count}건`
      ]);
      setStatus(`자동 제안 ${event.proposal.proposals.length}건 준비됨`);
      void refreshBrowse(event.root_id, browsePathRef.current);
      void refreshHistory(event.root_id);
    }).then((stop) => {
      if (cancelled) {
        stop();
      } else {
        unlistenAutoProposals = stop;
      }
    });

    return () => {
      cancelled = true;
      unlistenRootChanges?.();
      unlistenAutoProposals?.();
    };
  }, []);

  useEffect(() => {
    if (!window.__TAURI_INTERNALS__) return;

    let unlisten: (() => void) | undefined;
    let cancelled = false;
    listenForCleanlinessSnapshotUpdates((update) => {
      if (update.rootId !== selectedRootIdRef.current) return;
      setCleanlinessSnapshot(update.snapshot);
      setResultLines(formatCleanlinessLines(update.snapshot));
      setStatus(
        `Cleanliness ${update.snapshot.score}/100${update.syncQueued ? " (server sync queued)" : ""}`
      );
    }).then((stop) => {
      if (cancelled) stop();
      else unlisten = stop;
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
      setRoomSyncStates(bindingStates(storedRoots));
      setSelectedRootId((current) => current || storedRoots[0]?.root_id || "");
      const results = await Promise.allSettled(
        storedRoots
          .filter((root) => root.room_binding_status === "unbound")
          .map((root) => syncRootToMobile(root, false))
      );
      const failedCount = results.filter((result) => result.status === "rejected").length;
      if (failedCount > 0) {
        setStatus(`폴더 ${storedRoots.length}개 불러옴 · 휴대폰 동기화 ${failedCount}개 대기 중`);
      } else if (storedRoots.length > 0) {
        setStatus(`폴더 ${storedRoots.length}개를 휴대폰과 동기화했어요`);
      }
    } catch (caught) {
      setError(errorMessage(caught));
    }
  }

  async function reloadDurableRootBindings() {
    try {
      const storedRoots = await listManagedRoots();
      setRoots(storedRoots);
      setRoomSyncStates(bindingStates(storedRoots));
      const detachedIds = new Set(
        storedRoots
          .filter((root) => root.room_binding_status === "detached")
          .map((root) => root.root_id)
      );
      setWatchingRootIds(
        (current) => new Set([...current].filter((rootId) => !detachedIds.has(rootId)))
      );
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
    setStatus("데모 폴더 준비 중");
    try {
      const path = await prepareDemoRoot();
      setPathInput(path);
      setStatus("데모 폴더 준비 완료");
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("데모 준비 실패");
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
      "손상된 저널 파일을 격리하고 새 저널을 시작해요. " +
        "손상된 줄 이전에 기록된 작업은 앱에서 더 이상 되돌릴 수 없어요. 계속할까요?"
    );
    if (!confirmed) return;

    setError(null);
    setStatus("저널 복구 중");
    try {
      const report = await recoverJournal(selectedRootId);
      setResultLines([`손상된 저널을 ${report.quarantined_path}로 격리했어요`]);
      setStatus("저널 복구 완료");
      await refreshHistory(selectedRootId);
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("복구 실패");
    }
  }

  async function registerRoot() {
    setError(null);
    setStatus("폴더 등록 중");
    let managed: ManagedRoot;
    try {
      managed = await registerManagedRoot(pathInput.trim());
      const storedRoots = await listManagedRoots();
      setRoots(storedRoots);
      selectRoot(managed.root_id);
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("등록 실패");
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
      const storedRoots = await listManagedRoots();
      setRoots(storedRoots);
      if (announce) {
        setError(null);
        setStatus(room.created ? "폴더 등록 및 방 생성 완료" : "휴대폰과 동기화 완료");
      }
      return room;
    } catch (caught) {
      setRoomSyncStates((current) => ({ ...current, [root.root_id]: "failed" }));
      if (announce) {
        setError(`폴더는 로컬에 안전하게 저장됐지만, 휴대폰 방 동기화에 실패했어요: ${errorMessage(caught)}`);
        setStatus("휴대폰 방 동기화 대기");
      }
      throw caught;
    }
  }

  async function refreshLatestCleanliness(rootId = selectedRootId) {
    if (!rootId) {
      setCleanlinessSnapshot(null);
      return;
    }
    try {
      setCleanlinessSnapshot(await getLatestCleanlinessSnapshot(rootId));
    } catch (caught) {
      setError(errorMessage(caught));
    }
  }

  async function disconnectRootFromMobile(root: ManagedRoot) {
    setRoomSyncStates((current) => ({ ...current, [root.root_id]: "syncing" }));
    setError(null);
    setStatus("Checking mobile disconnect safety");
    try {
      const preflight = await preflightAgentRoomDisconnect(root.root_id);
      if (preflight.blocking_reasons.length > 0) {
        throw new Error(`ROOM_DISCONNECT_BLOCKED: ${preflight.blocking_reasons.join("; ")}`);
      }
      if (
        preflight.requires_confirmation &&
        !roomDisconnectAcknowledgements.current.has(root.root_id)
      ) {
        const confirmed = window.confirm(
          `This folder has ${preflight.undoable_operation_count} undoable local operation(s). ` +
            "Disconnecting stops mobile access and clears only the disposable file index; local files and undo history remain. Continue?"
        );
        if (!confirmed) {
          setRoomSyncStates((current) => ({ ...current, [root.root_id]: "synced" }));
          setStatus("Mobile disconnect cancelled");
          return;
        }
        roomDisconnectAcknowledgements.current.add(root.root_id);
      }
      roomDisconnectKeys.current[root.root_id] ??= newIdempotencyKey("room-disconnect");
      const report = await disconnectAgentRoom(
        root.root_id,
        roomDisconnectKeys.current[root.root_id],
        roomDisconnectAcknowledgements.current.has(root.root_id)
      );
      delete roomDisconnectKeys.current[root.root_id];
      roomDisconnectAcknowledgements.current.delete(root.root_id);
      setWatchingRootIds((current) => {
        const next = new Set(current);
        next.delete(root.root_id);
        return next;
      });
      await reloadDurableRootBindings();
      setResultLines([
        `Disconnected room ${report.room_id}`,
        "Local files and operation journal preserved",
        report.index_cleared ? "Disposable mobile browse index cleared" : "Browse index unchanged"
      ]);
      setStatus("Mobile folder disconnected");
    } catch (caught) {
      setRoomSyncStates((current) => ({ ...current, [root.root_id]: "failed" }));
      setError(errorMessage(caught));
      setStatus("Mobile disconnect failed; retry uses the same request key");
    }
  }

  async function updateSelectedRootState(patch: { enabled?: boolean; watch_on_startup?: boolean }) {
    if (!selectedRootId) return;

    setError(null);
    setStatus("폴더 설정 변경 중");
    try {
      const updated = await updateManagedRootState(selectedRootId, patch);
      setRoots((current) =>
        current.map((root) => (root.root_id === updated.root_id ? updated : root))
      );
      setStatus("폴더 설정 변경 완료");
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("폴더 설정 변경 실패");
    }
  }

  async function unregisterSelectedRoot() {
    if (!selectedRoot) return;

    const confirmed = window.confirm(
      `"${selectedRoot.display_name}" 폴더 관리를 해제할까요? 먼저 휴대폰 연결을 안전하게 해제한 뒤 로컬 등록을 지워요. 실제 파일과 작업 기록은 디스크에 남아요.`
    );
    if (!confirmed) return;

    const removedRootId = selectedRoot.root_id;
    setError(null);
    setStatus("폴더 연결 해제 중");
    try {
      rootUnregisterKeys.current[removedRootId] ??= newIdempotencyKey("root-unregister");
      const report = await unregisterManagedRoot(
        removedRootId,
        rootUnregisterKeys.current[removedRootId],
        true
      );
      delete rootUnregisterKeys.current[removedRootId];
      const remaining = await listManagedRoots();
      setRoots(remaining);
      setWatchingRootIds((current) => {
        const next = new Set(current);
        next.delete(removedRootId);
        return next;
      });
      // selectRoot resets the per-root workspace state (proposal, browse, history, ...).
      selectRoot(remaining[0]?.root_id ?? "");
      setStatus(
        report.server_room_removed
          ? "폴더 연결 해제 및 휴대폰 방 삭제 완료"
          : "폴더 연결 해제 완료"
      );
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("폴더 연결 해제 실패");
    }
  }

  async function updateSelectedAutoApprovalPolicy(
    patch: Parameters<typeof updateAutoApprovalPolicy>[1]
  ) {
    if (!selectedRootId) return;

    setError(null);
    setStatus("자동 승인 설정 변경 중");
    try {
      const policy = await updateAutoApprovalPolicy(selectedRootId, patch);
      setAutoApprovalPolicy(policy);
      setStatus("자동 승인 설정 변경 완료");
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("자동 승인 설정 변경 실패");
    }
  }

  async function analyzeSelectedRoot() {
    if (!selectedRootId) return;

    setError(null);
    setStatus("분석 중");
    try {
      const report = await analyzeRoot(selectedRootId);
      setResultLines([
        ...report.files.map((file) => `${file.path} (${file.size_bytes} bytes)`),
        ...formatSkippedEntries(report.skipped_entries)
      ]);
      setStatus(formatCountWithSkipped("분석 완료", report.files.length, report.skipped_entries.length, "개 파일"));
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("분석 실패");
    }
  }

  async function proposeForSelectedRoot() {
    if (!selectedRootId) return;
    const requestedRootId = selectedRootId;

    setError(null);
    setStatus("제안 만드는 중");
    try {
      const report = await proposeFileChanges(requestedRootId);
      if (selectedRootIdRef.current !== requestedRootId) return;
      proposalRef.current = report;
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
      setStatus(`제안 ${report.proposals.length}건 준비됨`);
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("제안 생성 실패");
    }
  }

  async function calculateSelectedCleanliness() {
    if (!selectedRootId) return;

    setError(null);
    setStatus("청결도 계산 중");
    try {
      const snapshot = await calculateCleanlinessSnapshot(selectedRootId);
      setCleanlinessSnapshot(snapshot);
      setResultLines(formatCleanlinessLines(snapshot));
      setStatus(`청결도 ${snapshot.score}/100`);
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("청결도 계산 실패");
    }
  }

  async function syncSelectedCleanliness() {
    if (!selectedRootId) return;

    setError(null);
    setStatus("청결도 동기화 중");
    try {
      const report = await submitCleanlinessSnapshot(selectedRootId);
      setCleanlinessSnapshot(report.snapshot);
      setRoomSyncStates((current) => ({ ...current, [report.root_id]: "synced" }));
      setResultLines([
        ...formatCleanlinessLines(report.snapshot),
        `방 | ${report.room_id}${report.room_created ? " (생성됨)" : ""}`,
        `스냅샷 | ${report.server_snapshot.snapshot_id}`
      ]);
      setStatus(`청결도 동기화 완료 ${report.snapshot.score}/100`);
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("청결도 동기화 실패");
    }
  }

  async function precheckSelectedRoot() {
    if (!selectedRootId || !proposal) return;

    const decisionsToApply = validatedDecisionEntries();
    if (!decisionsToApply) return;

    setError(null);
    setStatus("사전 점검 중");
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
          ? `사전 점검에서 제안 ${blocked.length}건 차단됨`
          : `승인된 제안 ${report.checks.length}건 사전 점검 완료`
      );
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("사전 점검 실패");
    }
  }

  async function autoApproveSelectedProposal() {
    if (!selectedRootId || !proposal) return;

    setError(null);
    setStatus("자동 승인 정책 적용 중");
    try {
      const autoDecisions = await autoApproveFileChanges(selectedRootId, proposal);
      if (autoDecisions.length === 0) {
        setStatus("자동 승인 대상 제안이 없어요");
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
      const precheck = await precheckFileChanges(selectedRootId, proposal, autoDecisions);
      if (proposalDecisionKey) {
        setPrecheckSnapshot({ key: proposalDecisionKey, report: precheck });
      }
      setResultLines([
        `제안 ${autoDecisions.length}건 자동 승인됨 — 여전히 사전 점검과 실행이 필요해요:`,
        ...autoApprovedItems.map((item) => `자동 승인 | ${item.from} -> ${item.to}`)
      ]);
      setStatus(`제안 ${autoDecisions.length}건 자동 승인됨`);
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("자동 승인 실패");
    }
  }

  async function executeSelectedRoot() {
    if (!selectedRootId || !proposal) return;

    const decisionsToApply = validatedDecisionEntries();
    if (!decisionsToApply) return;
    if (!proposalDecisionKey || !activePrecheck) {
      setError("실행 전에 최신 제안·결정 변경에 대해 사전 점검을 먼저 실행하세요.");
      setStatus("사전 점검 필요");
      return;
    }
    if (!activePrecheckReady) {
      setResultLines(formatPrecheckLines(activePrecheck));
      setStatus("사전 점검에 막혀 실행 불가");
      return;
    }

    setError(null);
    setStatus("실행 준비됨");
    try {
      const confirmed = window.confirm(
        `${selectedRoot?.display_name || "이 폴더"}의 승인된 파일 ${activePrecheck.checks.length}개를 옮길까요?`
      );
      if (!confirmed) {
        setStatus("실행 취소됨");
        return;
      }

      setStatus("실행 중");
      const autoApprovedCount = decisionsToApply.filter(
        (decision) =>
          decision.decision === "approved" && autoApprovedProposalIds.has(decision.proposal_id)
      ).length;
      const report = await executeFileChanges(selectedRootId, proposal, decisionsToApply);
      const lines = formatExecuteLines(report);
      setResultLines(lines);
      proposalRef.current = null;
      setProposal(null);
      setPrecheckSnapshot(null);
      setAutoApprovedProposalIds(new Set());
      setDecisions({});
      setRejectionReasons({});
      setStatus(
        `실행 ${report.executed_count} · 건너뜀 ${report.skipped_count} · 거절 ${report.rejected_count}` +
          (autoApprovedCount > 0 ? ` (자동 승인 ${autoApprovedCount})` : "")
      );
      await refreshHistory(selectedRootId);
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("실행 실패");
    }
  }

  async function undoSelectedRoot() {
    if (!selectedRootId) return;

    const confirmed = window.confirm(`${selectedRoot?.display_name || "이 폴더"}의 가장 최근 파일 작업을 되돌릴까요?`);
    if (!confirmed) return;

    setError(null);
    setStatus("되돌리는 중");
    try {
      const report = await undoLastFileOperation(selectedRootId);
      const lines = formatUndoLines(report);
      setResultLines(lines);
      setStatus(`되돌림 ${report.undone_count} · 건너뜀 ${report.skipped_count}`);
      await refreshHistory(selectedRootId);
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("되돌리기 실패");
    }
  }

  async function undoSelectedOperation(operation: OperationHistoryEntry) {
    if (!selectedRootId || !operation.can_undo) return;

    const confirmed = window.confirm(`${operation.from} -> ${operation.to} 작업을 되돌릴까요?`);
    if (!confirmed) return;

    setError(null);
    setStatus("선택한 작업 되돌리는 중");
    try {
      const report = await undoOperation(selectedRootId, operation.operation_id);
      const lines = formatUndoLines(report);
      setResultLines(lines);
      setStatus(`되돌림 ${report.undone_count} · 건너뜀 ${report.skipped_count}`);
      await refreshHistory(selectedRootId);
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("되돌리기 실패");
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
        setStatus("변경 감시 중지됨");
      } else {
        await startWatchingRoot(selectedRootId);
        setWatchingRootIds((current) => new Set(current).add(selectedRootId));
        setStatus("변경 감시 중");
      }
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("감시 전환 실패");
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
        setStatus(`${report.path || "/"} 탐색 중 (${report.skipped_entries.length}개 건너뜀)`);
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
      setStatus(formatCountWithSkipped("검색 결과", report.files.length, report.skipped_entries.length, "개 색인 파일"));
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("검색 실패");
    }
  }

  async function reindexSelectedRoot() {
    if (!selectedRootId) return;

    setError(null);
    setStatus("색인 다시 만드는 중");
    try {
      const report = await reindexManagedRoot(selectedRootId);
      setStatus(formatCountWithSkipped("색인 완료", report.files.length, report.skipped_entries.length, "개 파일"));
      // Refresh whatever the search box is currently showing against the new index.
      if (searchResults !== null) {
        await runSearch(selectedRootId, searchQuery);
      }
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("색인 재생성 실패");
    }
  }

  async function trashBrowseEntry(entry: BrowseEntry) {
    if (!selectedRootId || entry.is_dir) return;

    const confirmed = window.confirm(`${entry.path}를 복구 가능한 휴지통으로 옮길까요?`);
    if (!confirmed) return;

    setError(null);
    setStatus("파일을 휴지통으로 이동 중");
    try {
      setPrecheckSnapshot(null);
      const report = await trashFile(selectedRootId, entry.path);
      setResultLines([
        `휴지통 이동 | ${report.original_path} -> ${report.trashed_path}`,
        `메타데이터 | ${report.metadata_path}`,
        `작업 | ${report.operation_id}`
      ]);
      setStatus("파일을 복구 가능한 휴지통으로 옮겼어요");
      await refreshBrowse(selectedRootId, browsePath);
      await refreshHistory(selectedRootId);
      if (searchResults !== null) {
        await runSearch(selectedRootId, searchQuery);
      }
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("휴지통 이동 실패");
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
    // Event callbacks read the ref synchronously; update it before React commits state so an event
    // for the previously selected root cannot install a cross-root proposal during this transition.
    selectedRootIdRef.current = rootId;
    setSelectedRootId(rootId);
    proposalRef.current = null;
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
      setError("자동 승인은 최소 한 가지 동작을 허용해야 해요.");
      return;
    }

    void updateSelectedAutoApprovalPolicy({ allowed_actions: next });
  }

  function validatedDecisionEntries() {
    const missingReason = proposal?.proposals.find(
      (item) => decisions[item.proposal_id] === "rejected" && !rejectionReasons[item.proposal_id]?.trim()
    );

    if (missingReason) {
      setError(`거절한 제안에는 사유가 필요해요: ${missingReason.from}`);
      setStatus("결정이 올바르지 않음");
      return null;
    }

    if (commandDecisions.length === 0) {
      setError("계속하기 전에 제안을 최소 하나 승인하거나 거절하세요.");
      setStatus("선택된 결정 없음");
      return null;
    }

    return commandDecisions;
  }

  return (
    <div className={embedded ? "file-engine-panel embedded-file-engine" : "app-shell file-engine-panel"}>
      <section className="toolbar file-engine-topbar">
        <div>
          <h1>파일 정리</h1>
          <p>{status}</p>
        </div>
        <button type="button" onClick={refreshRoots}>
          새로고침
        </button>
      </section>

      <section className="panel root-register-panel">
        <label htmlFor="root-path">관리 폴더 경로</label>
        <div className="input-row">
          <input
            id="root-path"
            value={pathInput}
            onChange={(event) => setPathInput(event.target.value)}
            placeholder={demoRootHint}
          />
          <button type="button" onClick={browseForRoot}>
            폴더 찾기
          </button>
          <button type="button" onClick={() => void prepareDemoRootPath()}>
            데모
          </button>
          <button type="button" onClick={registerRoot} disabled={!pathInput.trim()}>
            등록
          </button>
        </div>
        <p className="path-text">
          경로를 입력하면 등록 버튼이 활성화돼요.
        </p>
      </section>

      <section className="workspace-grid file-engine-workspace">
        <div className="panel">
          <label htmlFor="root-select">등록된 폴더</label>
          <select
            id="root-select"
            value={selectedRootId}
            onChange={(event) => selectRoot(event.target.value)}
          >
            <option value="">선택된 폴더 없음</option>
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
                사용
              </label>
              <label>
                <input
                  type="checkbox"
                  checked={selectedRoot.watch_on_startup}
                  onChange={(event) =>
                    void updateSelectedRootState({ watch_on_startup: event.target.checked })
                  }
                />
                시작 시 감시
              </label>
              <small>{rootStatusLabel(selectedRoot.last_seen_status)}</small>
            </div>
          ) : null}
          {selectedRoot ? (
            <div className="agent-actions">
              <span className="path-text">
                휴대폰 방: {roomSyncLabel(roomSyncStates[selectedRoot.root_id])}
                {selectedRoot.room_id
                  ? ` (${selectedRoot.room_id})`
                  : selectedRoot.detached_room_id
                    ? ` (last ${selectedRoot.detached_room_id})`
                    : ""}
              </span>
              <button
                type="button"
                disabled={roomSyncStates[selectedRoot.root_id] === "syncing"}
                onClick={() => void syncRootToMobile(selectedRoot, true)}
              >
                {selectedRoot.room_binding_status === "detached"
                  ? "휴대폰 폴더 다시 연결"
                  : "휴대폰과 동기화"}
              </button>
              <button
                type="button"
                className="danger-button"
                onClick={() => void unregisterSelectedRoot()}
              >
                관리 폴더 해제
              </button>
              {selectedRoot.room_binding_status === "active" ? (
                <button
                  className="danger-button"
                  type="button"
                  disabled={roomSyncStates[selectedRoot.root_id] === "syncing"}
                  onClick={() => void disconnectRootFromMobile(selectedRoot)}
                >
                  {roomDisconnectKeys.current[selectedRoot.root_id]
                    ? "Retry mobile disconnect"
                    : "Disconnect mobile folder"}
                </button>
              ) : null}
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
                제안 자동 승인
              </label>
              <label>
                <input
                  type="checkbox"
                  checked={autoApprovalPolicy.allowed_actions.includes("trash")}
                  onChange={(event) => setAutoApprovalAction("trash", event.target.checked)}
                />
                휴지통
              </label>
              <label>
                <input
                  type="checkbox"
                  checked={autoApprovalPolicy.allowed_actions.includes("move")}
                  onChange={(event) => setAutoApprovalAction("move", event.target.checked)}
                />
                이동
              </label>
              <label>
                최대
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
                위 동작에 대해 준비된 제안 항목을 미리 점검해요. 자동 승인만으로는 파일 작업을 실행하지 않으며,
                아래의 사전 점검·실행이 여전히 필요해요. 이 정책은 휴대폰이나 에이전트가 시작한 제안에는 적용되지 않아요.
              </small>
            </div>
          ) : null}

          <div className="button-grid">
            <button type="button" onClick={analyzeSelectedRoot} disabled={!selectedRootId}>
              파일 분석
            </button>
            <button type="button" onClick={proposeForSelectedRoot} disabled={!selectedRootId}>
              정리 제안 만들기
            </button>
            <button
              type="button"
              onClick={() => void calculateSelectedCleanliness()}
              disabled={!selectedRootId}
            >
              청결도 계산
            </button>
            <button
              type="button"
              onClick={() => void syncSelectedCleanliness()}
              disabled={!selectedRootId}
            >
              청결도 동기화
            </button>
            <button
              type="button"
              onClick={precheckSelectedRoot}
              disabled={!selectedRootId || !proposal}
            >
              사전 점검
            </button>
            <button
              type="button"
              onClick={() => void autoApproveSelectedProposal()}
              disabled={!selectedRootId || !proposal || !autoApprovalPolicy?.enabled}
              title="준비된 제안만 정책으로 승인합니다. 실행 전 사전 점검은 그대로 필요합니다."
            >
              자동 승인 적용
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
              실행
            </button>
            <button
              type="button"
              onClick={undoSelectedRoot}
              disabled={!selectedRootId || !!journalCorruption}
            >
              되돌리기
            </button>
            <button
              type="button"
              onClick={() => void toggleWatchSelectedRoot()}
              disabled={!selectedRootId}
            >
              {isWatchingSelectedRoot ? "감시 중지" : "변경 감시"}
            </button>
          </div>
          {proposal ? (
            <p className="path-text">
              준비 {readyProposalCount}건, 승인 {approvedCount}건, 사전 점검{" "}
              {activePrecheckReady ? "완료" : activePrecheck ? "차단됨" : "필요"}
            </p>
          ) : null}
          {cleanlinessSnapshot ? (
            <p className="path-text">
              청결도 {cleanlinessSnapshot.score}/100 | 전체{" "}
              {cleanlinessSnapshot.metrics.totalFileCount}개 중 미정리{" "}
              {cleanlinessSnapshot.metrics.unorganizedFileCount}개 | {cleanlinessSnapshot.formulaVersion}
            </p>
          ) : null}
          <p className="mode-note">
            모바일, 에이전트, AI 요청은 정리 제안만 만들 수 있습니다. 실제 파일 변경은 이 PC의 승인과 사전 점검을 거쳐 실행됩니다.
          </p>
          {journalCorruption ? (
            <p className="error-text">실행/되돌리기 전에 작업 기록 복구가 필요합니다.</p>
          ) : null}
          {isWatchingSelectedRoot ? (
            <p className="path-text">변경 감시 중입니다. 파일 목록과 기록이 자동으로 새로고침됩니다.</p>
          ) : null}
        </div>

        <div className="panel">
          <h2>정리 제안</h2>
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
                      placeholder="거절 사유"
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
                    승인
                  </option>
                  <option value="pending">보류</option>
                  <option value="rejected">거절</option>
                </select>
              </article>
            )) || <p>불러온 제안이 없습니다.</p>}
          </div>
        </div>
      </section>

      <section className="panel search-panel">
        <div className="section-header">
          <h2>검색</h2>
          <button type="button" onClick={reindexSelectedRoot} disabled={!selectedRootId}>
            색인 새로 만들기
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
            placeholder="파일 이름으로 검색"
            disabled={!selectedRootId}
          />
          <button type="submit" disabled={!selectedRootId}>
            검색
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
          {searchResults?.length === 0 ? <p>일치하는 색인 파일이 없습니다.</p> : null}
          {searchResults === null && selectedRootId ? (
            <p>색인을 만든 뒤 관리 폴더의 파일을 검색할 수 있습니다. 변경 감시를 켜면 색인이 자동으로 갱신됩니다.</p>
          ) : null}
          {!selectedRootId ? <p>검색할 폴더를 먼저 선택하세요.</p> : null}
        </div>
      </section>

      <section className="panel browse-panel">
        <div className="section-header">
          <div>
            <h2>파일 탐색</h2>
            <p className="path-text">
              이 영역의 작업은 현재 PC에서 사용자가 직접 누를 때만 실행됩니다.
            </p>
          </div>
          <button type="button" onClick={() => void refreshBrowse()} disabled={!selectedRootId}>
            새로고침
          </button>
        </div>
        <nav className="breadcrumb">
          <button type="button" onClick={() => setBrowsePath("")} disabled={!selectedRootId}>
            {selectedRoot?.display_name || "루트"}
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
                  휴지통
                </button>
              ) : null}
            </article>
          ))}
          {selectedRootId && browseEntries.length === 0 ? <p>이 폴더는 비어 있습니다.</p> : null}
          {!selectedRootId ? <p>탐색할 폴더를 먼저 선택하세요.</p> : null}
        </div>
      </section>

      <section className="panel output-panel">
        <h2>결과</h2>
        {error ? <p className="error-text">{error}</p> : null}
        <pre>{resultLines.join("\n") || "아직 출력이 없습니다."}</pre>
      </section>

      <section className="panel history-panel">
        <div className="section-header">
          <h2>작업 기록</h2>
          <button type="button" onClick={() => void refreshHistory()} disabled={!selectedRootId}>
            새로고침
          </button>
        </div>
        {journalCorruption ? (
          <div className="recovery-banner">
            <p>
              <strong>복구가 필요합니다.</strong> 작업 기록 {journalCorruption.line}번째 줄부터 읽을 수 없습니다:{" "}
              {journalCorruption.message}
            </p>
            <p>
              손상된 기록을 격리하기 전까지 새 실행과 되돌리기는 막힙니다. 격리 후에는 손상 지점 이전 작업을 앱에서 되돌릴 수 없습니다.
            </p>
            <button type="button" onClick={() => void recoverJournalForSelectedRoot()}>
              작업 기록 복구
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
                <small className="reason-text">되돌릴 수 없음: {operation.undo_blocked_reason}</small>
              ) : null}
              <button
                type="button"
                onClick={() => void undoSelectedOperation(operation)}
                disabled={!operation.can_undo || !!journalCorruption}
              >
                되돌리기
              </button>
            </article>
          ))}
          {history.length === 0 ? <p>아직 작업 기록이 없습니다.</p> : null}
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
  return `${proposalStatusLabel(item.status)} | ${item.from} -> ${item.to} | ${item.reason}`;
}

function formatCleanlinessLines(snapshot: CleanlinessSnapshot) {
  return [
    `formula | ${snapshot.formulaVersion}`,
    `점수 | ${snapshot.score}/100`,
    `파일 | 전체 ${snapshot.metrics.totalFileCount}, 관리됨 ${snapshot.metrics.managedFileCount}, 미정리 ${snapshot.metrics.unorganizedFileCount}`,
    `계산 시각 | ${snapshot.calculatedAt}`,
    ...snapshot.metrics.deductions.map(
      (deduction) =>
        `감점 | ${deduction.reasonCode} | ${deduction.count}건 | -${deduction.points}`
    )
  ];
}

function formatExecuteLines(report: ExecuteReport) {
  return ["executed", "skipped", "rejected"].flatMap((status) => {
    const rows = report.results.filter((result) => result.status === status);
    if (rows.length === 0) return [];
    return [
      `[${executeStatusLabel(status as ExecuteReport["results"][number]["status"])}]`,
      ...rows.map((result) =>
        [executeStatusLabel(result.status), `${result.from} -> ${result.to}`, result.reason].filter(Boolean).join(" | ")
      )
    ];
  });
}

function formatUndoLines(report: UndoReport) {
  return report.results.map((result) =>
    [undoStatusLabel(result.status), `${result.from} -> ${result.to}`, result.reason].filter(Boolean).join(" | ")
  );
}

function formatPrecheckLines(report: PrecheckReport) {
  return report.checks.map((check) =>
    [precheckStatusLabel(check.status), `${check.from} -> ${check.to}`, check.reason].filter(Boolean).join(" | ")
  );
}

function formatPrecheckSummary(report: PrecheckReport, item: Proposal) {
  const check = report.checks.find((entry) => entry.from === item.from && entry.to === item.to);
  return check ? `사전 점검: ${precheckStatusLabel(check.status)}` : "사전 점검: 선택 안 됨";
}

function proposalSnapshotKey(
  rootId: string,
  proposal: ProposalReport,
  decisions: DecisionEntry[]
) {
  return JSON.stringify({ rootId, proposal, decisions });
}

function formatSkippedEntries(entries: Array<{ path: string; reason: string }>) {
  return entries.map((entry) => `건너뜀 | ${entry.path || "/"} | ${entry.reason}`);
}

function formatCountWithSkipped(label: string, count: number, skipped: number, noun: string) {
  return skipped > 0 ? `${label} ${count} ${noun}, 건너뜀 ${skipped}` : `${label} ${count} ${noun}`;
}

function errorMessage(caught: unknown) {
  return caught instanceof Error ? caught.message : String(caught);
}

function bindingStates(roots: ManagedRoot[]): Record<string, RoomSyncState> {
  return Object.fromEntries(
    roots.map((root) => [
      root.root_id,
      root.room_binding_status === "active"
        ? "synced"
        : root.room_binding_status === "detached"
          ? "detached"
          : "unbound"
    ])
  );
}

function newIdempotencyKey(prefix: string) {
  const nonce = globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(36).slice(2)}`;
  return `${prefix}-${nonce}`;
}

function rootStatusLabel(status: ManagedRoot["last_seen_status"]) {
  return { ready: "정상", missing: "폴더 없음", error: "오류" }[status] ?? status;
}

function roomSyncLabel(state: RoomSyncState | undefined) {
  switch (state) {
    case "syncing":
      return "동기화 중";
    case "synced":
      return "동기화됨";
    case "failed":
      return "실패";
    case "detached":
      return "연결 해제됨";
    case "unbound":
      return "연결 대기";
    default:
      return "대기 중";
  }
}

function proposalStatusLabel(status: Proposal["status"]) {
  return { ready: "준비됨", destination_exists: "대상 있음" }[status] ?? status;
}

function executeStatusLabel(status: ExecuteReport["results"][number]["status"]) {
  return { executed: "실행됨", skipped: "건너뜀", rejected: "거절됨" }[status] ?? status;
}

function undoStatusLabel(status: UndoReport["results"][number]["status"]) {
  return { undone: "되돌림", skipped: "건너뜀" }[status] ?? status;
}

function precheckStatusLabel(status: PrecheckReport["checks"][number]["status"]) {
  return {
    ready: "준비됨",
    destination_exists: "대상 있음",
    missing_source: "원본 없음",
    source_changed: "원본 변경됨",
    rejected_path: "경로 거부됨"
  }[status] ?? status;
}
