import { useEffect, useRef, useState } from "react";
import type { PointerEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import {
  approveAutoCleanupProposalFromChat,
  listenForAutoCleanupProposals,
  listManagedRoots
} from "../files/fileEngineApi";
import type { AutoCleanupProposalEvent, ExecuteReport, ManagedRoot } from "../files/fileEngineApi";
import {
  approveAgentCommandDraftAndExecute,
  approveAgentOpenProposalAndExecute,
  createAgentChatQuickCleanup,
  createAgentChatSession,
  getBackgroundRuntimeStatus,
  getAgentChatQuickView,
  listAgentOpenProposals,
  listAgentChatMessages,
  listAgentChatSessions,
  markAgentChatSessionRead,
  sendAgentChatMessage
} from "../agent/agentApi";
import type {
  AgentChatMessage,
  AgentChatQuickView,
  AgentChatSession,
  DecisionProcessingReport,
  AgentOpenProposal
} from "../agent/agentApi";
import {
  CharacterEvent,
  hideChatOverlay,
  listenForChatAutoProposals,
  listenForCharacterEvents,
  listenForChatRoomSelection,
  readChatAutoProposals,
  readChatRoomSelection
} from "./overlayApi";

const chatAvatarUrl = new URL(
  "../../../../../packages/character-assets/new_mouse/mouse_walk_clean.png",
  import.meta.url
).href;

type LocalAutoProposal = AutoCleanupProposalEvent & {
  localKey: string;
  receivedAt: number;
};

type TimedOpenProposal = AgentOpenProposal & {
  receivedAt: number;
};

type ChatTimelineItem =
  | {
      kind: "message";
      key: string;
      timestamp: number;
      message: AgentChatMessage;
    }
  | {
      kind: "openProposal";
      key: string;
      timestamp: number;
      proposal: TimedOpenProposal;
    }
  | {
      kind: "localProposal";
      key: string;
      timestamp: number;
      proposal: LocalAutoProposal;
    };

type PersistedChatActionState = {
  approvedDraftIds: string[];
  dismissedLocalProposalKeys: string[];
  proposalReceivedAtByKey: Record<string, number>;
  skippedDraftIds: string[];
  skippedLocalProposalKeys: string[];
  skippedProposalIds: string[];
  submittedDraftIds: string[];
  submittedLocalProposalKeys: string[];
  submittedProposalIds: string[];
};

const CHAT_ACTION_STATE_STORAGE_KEY = "mousekeeper.chatOverlay.actionState.v1";

const fallbackQuickPrompts = [
  {
    id: "find-reports",
    label: "Find reports",
    prompt: "최근 보고서나 리포트 파일을 찾아줘."
  },
  {
    id: "clean-downloads",
    label: "Clean Downloads",
    prompt: "다운로드 폴더에서 정리할 만한 파일을 제안해줘."
  },
  {
    id: "pdf-rule",
    label: "PDF rule",
    prompt: "PDF 파일을 정리하는 규칙을 만들어줘."
  },
  {
    id: "move-screenshots",
    label: "Move screenshots",
    prompt: "스크린샷 파일을 한 폴더로 옮기는 제안을 만들어줘."
  }
];

const emptyChatActionState: PersistedChatActionState = {
  approvedDraftIds: [],
  dismissedLocalProposalKeys: [],
  proposalReceivedAtByKey: {},
  skippedDraftIds: [],
  skippedLocalProposalKeys: [],
  skippedProposalIds: [],
  submittedDraftIds: [],
  submittedLocalProposalKeys: [],
  submittedProposalIds: []
};

function setFromArray(values: readonly string[] | undefined) {
  return new Set(values?.filter((value): value is string => typeof value === "string") ?? []);
}

function readPersistedChatActionState(): PersistedChatActionState {
  if (typeof window === "undefined") return emptyChatActionState;
  try {
    const raw = window.localStorage.getItem(CHAT_ACTION_STATE_STORAGE_KEY);
    if (!raw) return emptyChatActionState;
    const parsed = JSON.parse(raw) as Partial<PersistedChatActionState>;
    return {
      approvedDraftIds: arrayField(parsed.approvedDraftIds),
      dismissedLocalProposalKeys: arrayField(parsed.dismissedLocalProposalKeys),
      proposalReceivedAtByKey: numberRecordField(parsed.proposalReceivedAtByKey),
      skippedDraftIds: arrayField(parsed.skippedDraftIds),
      skippedLocalProposalKeys: arrayField(parsed.skippedLocalProposalKeys),
      skippedProposalIds: arrayField(parsed.skippedProposalIds),
      submittedDraftIds: arrayField(parsed.submittedDraftIds),
      submittedLocalProposalKeys: arrayField(parsed.submittedLocalProposalKeys),
      submittedProposalIds: arrayField(parsed.submittedProposalIds)
    };
  } catch (cause) {
    console.warn("Failed to read persisted chat action state", cause);
    return emptyChatActionState;
  }
}

function writePersistedChatActionState(state: PersistedChatActionState) {
  try {
    window.localStorage.setItem(CHAT_ACTION_STATE_STORAGE_KEY, JSON.stringify(state));
  } catch (cause) {
    console.warn("Failed to persist chat action state", cause);
  }
}

function arrayField(value: unknown) {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];
}

function numberRecordField(value: unknown) {
  if (!value || typeof value !== "object" || Array.isArray(value)) return {};
  return Object.fromEntries(
    Object.entries(value)
      .filter((entry): entry is [string, number] => typeof entry[1] === "number" && Number.isFinite(entry[1]))
  );
}

/**
 * The chat lives in its own fixed-size window (`chat-overlay`) instead of resizing the mascot
 * window. It loads its own room context, so the mascot window only has to show/hide it.
 */
export function ChatOverlay() {
  const persistedActionStateRef = useRef(readPersistedChatActionState());
  const [event, setEvent] = useState<CharacterEvent>({ kind: "IDLE" });
  const [rooms, setRooms] = useState<ManagedRoot[]>([]);
  const [chatSession, setChatSession] = useState<AgentChatSession | null>(null);
  const [quickView, setQuickView] = useState<AgentChatQuickView | null>(null);
  const [chatMessages, setChatMessages] = useState<AgentChatMessage[]>([]);
  const [localStatusMessages, setLocalStatusMessages] = useState<AgentChatMessage[]>([]);
  const [draft, setDraft] = useState("");
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [answerPending, setAnswerPending] = useState(false);
  const [chatLoading, setChatLoading] = useState(false);
  const [approvingDraftIds, setApprovingDraftIds] = useState<Set<string>>(new Set());
  const [approvedDraftIds, setApprovedDraftIds] = useState<Set<string>>(
    () => setFromArray(persistedActionStateRef.current.approvedDraftIds)
  );
  const [submittedDraftIds, setSubmittedDraftIds] = useState<Set<string>>(
    () => setFromArray(persistedActionStateRef.current.submittedDraftIds)
  );
  const [skippedDraftIds, setSkippedDraftIds] = useState<Set<string>>(
    () => setFromArray(persistedActionStateRef.current.skippedDraftIds)
  );
  const [openProposals, setOpenProposals] = useState<TimedOpenProposal[]>([]);
  const [approvingProposalIds, setApprovingProposalIds] = useState<Set<string>>(new Set());
  const [submittedProposalIds, setSubmittedProposalIds] = useState<Set<string>>(
    () => setFromArray(persistedActionStateRef.current.submittedProposalIds)
  );
  const [skippedProposalIds, setSkippedProposalIds] = useState<Set<string>>(
    () => setFromArray(persistedActionStateRef.current.skippedProposalIds)
  );
  const [localAutoProposals, setLocalAutoProposals] = useState<LocalAutoProposal[]>([]);
  const [approvingLocalProposalKeys, setApprovingLocalProposalKeys] = useState<Set<string>>(new Set());
  const [submittedLocalProposalKeys, setSubmittedLocalProposalKeys] = useState<Set<string>>(
    () => setFromArray(persistedActionStateRef.current.submittedLocalProposalKeys)
  );
  const [skippedLocalProposalKeys, setSkippedLocalProposalKeys] = useState<Set<string>>(
    () => setFromArray(persistedActionStateRef.current.skippedLocalProposalKeys)
  );
  const [dismissedLocalProposalKeys, setDismissedLocalProposalKeys] = useState<Set<string>>(
    () => setFromArray(persistedActionStateRef.current.dismissedLocalProposalKeys)
  );
  const [proposalReceivedAtByKey, setProposalReceivedAtByKey] = useState<Record<string, number>>(
    () => persistedActionStateRef.current.proposalReceivedAtByKey
  );
  const dismissedLocalProposalKeysRef = useRef(
    setFromArray(persistedActionStateRef.current.dismissedLocalProposalKeys)
  );
  const proposalReceivedAtByKeyRef = useRef(persistedActionStateRef.current.proposalReceivedAtByKey);
  const suppressedOpenProposalRoomIdsRef = useRef(new Set<string>());
  const [selectedRootId, setSelectedRootId] = useState<string | null>(
    () => readChatRoomSelection().rootId
  );

  const selectedRoom = selectedRootId
    ? rooms.find((root) => root.root_id === selectedRootId) ?? null
    : null;
  const activeRoom =
    selectedRoom ?? rooms.find((root) => root.room_binding_status === "active") ?? rooms[0] ?? null;
  const activeRoomName = activeRoom?.display_name ?? "방 없음";
  const activeRoomId = activeRoom?.room_id ?? null;
  const activeRootId = activeRoom?.root_id ?? null;
  const visibleLocalAutoProposals = (
    activeRootId
      ? localAutoProposals.filter((proposal) => proposal.root_id === activeRootId)
      : localAutoProposals
  ).filter((proposal) => proposal.proposal.proposals.length > 0);
  const visibleOpenProposals = visibleLocalAutoProposals.length > 0 ? [] : openProposals;
  const displayedMessages = mergeMessagesByTime(chatMessages, localStatusMessages);

  function receivedAtForProposalKey(storageKey: string) {
    const current = proposalReceivedAtByKeyRef.current[storageKey];
    if (current) return current;
    const receivedAt = Date.now();
    proposalReceivedAtByKeyRef.current = {
      ...proposalReceivedAtByKeyRef.current,
      [storageKey]: receivedAt
    };
    setProposalReceivedAtByKey(proposalReceivedAtByKeyRef.current);
    return receivedAt;
  }

  async function loadRooms() {
    try {
      const storedRooms = await listManagedRoots();
      setRooms((current) =>
        storedRooms.length === 0 && current.length > 0 ? current : storedRooms
      );
    } catch (cause) {
      console.error("Failed to load chat overlay roots", cause);
    }
  }

  function rememberLocalAutoProposal(proposal: AutoCleanupProposalEvent) {
    if (proposal.proposal.proposals.length === 0) return;
    const localKey = localAutoProposalKey(proposal);
    const receivedAt = receivedAtForProposalKey(`local:${localKey}`);
    setLocalAutoProposals((current) =>
      [
        { ...proposal, localKey, receivedAt },
        ...current.filter((item) => item.localKey !== localKey)
      ].slice(0, 20)
    );
  }

  async function loadLatestAutoCleanupProposals() {
    try {
      for (const proposal of readChatAutoProposals()) {
        rememberLocalAutoProposal(proposal);
      }
      const status = await getBackgroundRuntimeStatus();
      for (const proposal of status.last_auto_cleanup_proposals) {
        rememberLocalAutoProposal(proposal);
      }
    } catch (cause) {
      console.error("Failed to load latest auto cleanup proposals", cause);
    }
  }

  async function loadOpenProposals(roomId = activeRoomId) {
    if (!roomId) {
      setOpenProposals([]);
      return;
    }
    if (suppressedOpenProposalRoomIdsRef.current.has(roomId)) {
      setOpenProposals([]);
      return;
    }
    try {
      const proposals = await listAgentOpenProposals(roomId);
      const open = proposals.filter((proposal) => proposal.status === "OPEN");
      setOpenProposals(
        open.map((proposal) => ({
          ...proposal,
          receivedAt: receivedAtForProposalKey(`open:${proposal.proposal_id}`)
        }))
      );
    } catch (cause) {
      console.error("Failed to load open chat proposals", cause);
    }
  }

  useEffect(() => {
    const unlisten = listenForCharacterEvents(setEvent);
    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

  useEffect(() => {
    void loadRooms();
    void loadLatestAutoCleanupProposals();
  }, []);

  useEffect(() => {
    dismissedLocalProposalKeysRef.current = dismissedLocalProposalKeys;
  }, [dismissedLocalProposalKeys]);

  useEffect(() => {
    proposalReceivedAtByKeyRef.current = proposalReceivedAtByKey;
  }, [proposalReceivedAtByKey]);

  useEffect(() => {
    writePersistedChatActionState({
      approvedDraftIds: Array.from(approvedDraftIds),
      dismissedLocalProposalKeys: Array.from(dismissedLocalProposalKeys),
      proposalReceivedAtByKey,
      skippedDraftIds: Array.from(skippedDraftIds),
      skippedLocalProposalKeys: Array.from(skippedLocalProposalKeys),
      skippedProposalIds: Array.from(skippedProposalIds),
      submittedDraftIds: Array.from(submittedDraftIds),
      submittedLocalProposalKeys: Array.from(submittedLocalProposalKeys),
      submittedProposalIds: Array.from(submittedProposalIds)
    });
  }, [
    approvedDraftIds,
    dismissedLocalProposalKeys,
    proposalReceivedAtByKey,
    skippedDraftIds,
    skippedLocalProposalKeys,
    skippedProposalIds,
    submittedDraftIds,
    submittedLocalProposalKeys,
    submittedProposalIds
  ]);

  useEffect(() => {
    if (!window.__TAURI_INTERNALS__) return;
    const unlisten = listenForChatRoomSelection((selection) => {
      setSelectedRootId(selection.rootId);
      void loadRooms();
    });
    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

  useEffect(() => {
    if (!window.__TAURI_INTERNALS__) return;
    const unlisten = getCurrentWindow().onFocusChanged((focusEvent) => {
      if (focusEvent.payload) {
        void loadRooms();
        void loadLatestAutoCleanupProposals();
        void loadOpenProposals();
      }
    });
    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

  useEffect(() => {
    if (!selectedRootId || rooms.length === 0) return;
    if (rooms.some((root) => root.root_id === selectedRootId)) return;
    setSelectedRootId(null);
  }, [rooms, selectedRootId]);

  useEffect(() => {
    if (!activeRoomId) {
      setChatSession(null);
      setQuickView(null);
      setChatMessages([]);
      setLocalStatusMessages([]);
      setOpenProposals([]);
      setNotice("먼저 데스크탑에서 관리 폴더를 서버 room과 연결해야 채팅을 이어서 기록할 수 있어요.");
      return;
    }

    let cancelled = false;
    setChatLoading(true);
    setNotice(null);
    void ensureRoomChatSession(activeRoomId, activeRoomName)
      .then(async (session) => {
        const [messages, quick] = await Promise.all([
          listAgentChatMessages(session.session_id),
          getAgentChatQuickView(activeRoomId).catch(() => null)
        ]);
        await markAgentChatSessionRead(session.session_id, lastAgentChatMessageId(messages)).catch(() => undefined);
        if (cancelled) return;
        setChatSession(session);
        setQuickView(quick);
        setChatMessages(messages);
        setLocalStatusMessages([]);
        void loadOpenProposals(activeRoomId);
      })
      .catch((cause) => {
        if (cancelled) return;
        setChatSession(null);
        setQuickView(null);
        setChatMessages([]);
        setNotice(cause instanceof Error ? cause.message : String(cause));
      })
      .finally(() => {
        if (!cancelled) setChatLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [activeRoomId, activeRoomName]);

  useEffect(() => {
    if (event.kind === "WAITING_APPROVAL") {
      void loadOpenProposals();
    }
  }, [event.kind, activeRoomId]);

  useEffect(() => {
    const unlistenBridge = listenForChatAutoProposals((proposal) => {
      rememberLocalAutoProposal(proposal);
    });
    const unlisten = listenForAutoCleanupProposals((proposal) => {
      rememberLocalAutoProposal(proposal);
      window.setTimeout(() => void loadOpenProposals(), 1000);
      window.setTimeout(() => void loadOpenProposals(), 3500);
    });
    return () => {
      void unlistenBridge.then((off) => off());
      void unlisten.then((off) => off());
    };
  }, [activeRoomId]);

  useEffect(() => {
    if (!chatSession || answerPending) return;
    const timer = window.setInterval(() => {
      void listAgentChatMessages(chatSession.session_id)
        .then(async (messages) => {
          await markAgentChatSessionRead(chatSession.session_id, lastAgentChatMessageId(messages)).catch(() => undefined);
          setChatMessages(messages);
        })
        .catch(() => undefined);
    }, 5000);
    return () => window.clearInterval(timer);
  }, [answerPending, chatSession]);

  function pushLocalStatusMessage(message: AgentChatMessage) {
    setLocalStatusMessages((items) => appendUniqueMessages(items, [message]));
  }

  async function submitDraft() {
    const trimmed = draft.trim();
    if (!trimmed || !activeRoomId) {
      setNotice("먼저 관리 폴더를 서버 room과 연결해야 채팅을 보낼 수 있어요.");
      return;
    }
    const optimisticId = `local-${globalThis.crypto?.randomUUID?.() ?? Date.now()}`;
    const optimisticMessage: AgentChatMessage = {
      message_id: optimisticId,
      room_id: activeRoomId,
      session_id: chatSession?.session_id ?? null,
      sender_type: "USER",
      message_type: "TEXT",
      content: trimmed,
      structured_payload: null,
      command_id: null,
      created_at: new Date().toISOString()
    };
    setNotice(null);
    setBusy(true);
    setAnswerPending(true);
    setDraft("");
    if (activeRoomId) {
      suppressedOpenProposalRoomIdsRef.current.delete(activeRoomId);
    }
    setChatMessages((items) => appendUniqueMessages(items, [optimisticMessage]));
    try {
      const session = chatSession ?? (await ensureRoomChatSession(activeRoomId, activeRoomName));
      if (!chatSession) setChatSession(session);
      const result = await sendAgentChatMessage(session.session_id, trimmed);
      const sentMessages = [result.message, result.assistant].filter(isChatMessage);
      await markAgentChatSessionRead(session.session_id, lastAgentChatMessageId(sentMessages)).catch(() => undefined);
      setChatMessages((items) =>
        appendUniqueMessages(
          items.filter((message) => message.message_id !== optimisticId),
          sentMessages
        )
      );
      setNotice(aiNotice(result.ai_status));
    } catch (cause) {
      setChatMessages((items) => items.filter((message) => message.message_id !== optimisticId));
      setDraft((current) => current || trimmed);
      setNotice(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
      setAnswerPending(false);
    }
  }

  async function runQuickCleanup() {
    setNotice(null);
    setBusy(true);
    try {
      if (!activeRoomId) {
        throw new Error("관리 폴더를 서버 room과 연결해야 빠른 정리 제안을 만들 수 있어요.");
      }
      suppressedOpenProposalRoomIdsRef.current.delete(activeRoomId);
      const result = await createAgentChatQuickCleanup(activeRoomId);
      setChatSession(result.session);
      const sentMessages = [result.message, result.assistant].filter(isChatMessage);
      setChatMessages((items) => appendUniqueMessages(items, sentMessages));
      await markAgentChatSessionRead(result.session.session_id, lastAgentChatMessageId(sentMessages)).catch(() => undefined);
      const quick = await getAgentChatQuickView(activeRoomId).catch(() => null);
      setQuickView(quick);
      setNotice("빠른 정리 제안 카드가 추가됐어요. 승인하면 PC 분석이 시작됩니다.");
    } catch (cause) {
      setNotice(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function approveCommandDraft(draftId: string) {
    if (!activeRoomId) {
      setNotice("관리 폴더를 서버 room과 연결해야 명령을 실행할 수 있어요.");
      return;
    }
    const idempotencyKey = commandDraftApprovalKey(draftId);
    setNotice(null);
    setBusy(true);
    setSubmittedDraftIds((current) => new Set(current).add(draftId));
    setApprovingDraftIds((current) => new Set(current).add(draftId));
    try {
      const report = await approveAgentCommandDraftAndExecute(draftId, activeRoomId, idempotencyKey);
      const executed = report.execution_report.executed_item_count;
      const skipped = report.execution_report.skipped_item_count;
      setApprovedDraftIds((current) => new Set(current).add(draftId));
      if (isDecisionSkippedOnly(report.execution_report)) {
        setSkippedDraftIds((current) => new Set(current).add(draftId));
      }
      pushLocalStatusMessage(
        executionStatusMessage({
          roomId: activeRoomId,
          sessionId: chatSession?.session_id ?? null,
          content: formatDecisionExecutionResult("요청한 작업 실행 결과", report.execution_report)
        })
      );
      setNotice(`승인 후 실행 완료: 실행 ${executed}개, 건너뜀 ${skipped}개`);
      if (chatSession) {
        const messages = await listAgentChatMessages(chatSession.session_id);
        setChatMessages(messages);
        await markAgentChatSessionRead(chatSession.session_id, lastAgentChatMessageId(messages)).catch(() => undefined);
      }
      const quick = await getAgentChatQuickView(activeRoomId).catch(() => null);
      setQuickView(quick);
      await loadOpenProposals(activeRoomId);
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause);
      setNotice(
        message.includes("IDEMPOTENCY_CONFLICT")
          ? "이 명령 초안은 이전 승인 시도와 충돌했어요. 같은 요청을 한 번 더 보내 새 초안을 만든 뒤 승인해 주세요."
          : message
      );
    } finally {
      setBusy(false);
      setApprovingDraftIds((current) => {
        const next = new Set(current);
        next.delete(draftId);
        return next;
      });
    }
  }

  async function approveOpenProposal(proposalId: string) {
    if (!activeRoomId) {
      setNotice("관리 폴더를 서버 room과 연결해야 제안을 실행할 수 있어요.");
      return;
    }
    const idempotencyKey = proposalApprovalKey(proposalId);
    setNotice(null);
    setBusy(true);
    setSubmittedProposalIds((current) => new Set(current).add(proposalId));
    setApprovingProposalIds((current) => new Set(current).add(proposalId));
    try {
      const report = await approveAgentOpenProposalAndExecute(proposalId, activeRoomId, idempotencyKey);
      const executed = report.execution_report.executed_item_count;
      const skipped = report.execution_report.skipped_item_count;
      if (isDecisionSkippedOnly(report.execution_report)) {
        setSkippedProposalIds((current) => new Set(current).add(proposalId));
      }
      pushLocalStatusMessage(
        executionStatusMessage({
          roomId: activeRoomId,
          sessionId: chatSession?.session_id ?? null,
          content: formatDecisionExecutionResult("자동 제안 실행 결과", report.execution_report)
        })
      );
      setNotice(`자동 제안 실행 완료: 실행 ${executed}개, 건너뜀 ${skipped}개`);
      if (activeRootId) {
        setLocalAutoProposals((current) =>
          current.filter((proposal) => proposal.root_id !== activeRootId)
        );
      }
      await loadOpenProposals(activeRoomId);
      const quick = await getAgentChatQuickView(activeRoomId).catch(() => null);
      setQuickView(quick);
    } catch (cause) {
      setNotice(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
      setApprovingProposalIds((current) => {
        const next = new Set(current);
        next.delete(proposalId);
        return next;
      });
    }
  }

  async function approveLocalAutoProposal(localKey: string) {
    const proposal = localAutoProposals.find((item) => item.localKey === localKey);
    if (!proposal) {
      setNotice("실행할 자동 제안을 찾을 수 없어요. 다음 자동 점검을 기다려 주세요.");
      return;
    }
    setNotice(null);
    setBusy(true);
    setSubmittedLocalProposalKeys((current) => new Set(current).add(localKey));
    setApprovingLocalProposalKeys((current) => new Set(current).add(localKey));
    try {
      const serverProposal = activeRoomId ? await singleOpenProposalForRoom(activeRoomId, openProposals) : null;
      if (serverProposal && activeRoomId) {
        setSubmittedProposalIds((current) => new Set(current).add(serverProposal.proposal_id));
        const report = await approveAgentOpenProposalAndExecute(
          serverProposal.proposal_id,
          activeRoomId,
          proposalApprovalKey(serverProposal.proposal_id)
        );
        const skippedOnly = isDecisionSkippedOnly(report.execution_report);
        if (skippedOnly) {
          setSkippedLocalProposalKeys((current) => new Set(current).add(localKey));
          setSkippedProposalIds((current) => new Set(current).add(serverProposal.proposal_id));
        }
        setOpenProposals([]);
        const executed = report.execution_report.executed_item_count;
        const skipped = report.execution_report.skipped_item_count;
        pushLocalStatusMessage(
          executionStatusMessage({
            roomId: activeRoomId,
            sessionId: chatSession?.session_id ?? null,
            content: formatDecisionExecutionResult("자동 제안 실행 결과", report.execution_report)
          })
        );
        setNotice(`자동 제안 실행 완료: 실행 ${executed}개, 건너뜀 ${skipped}개`);
        const quick = await getAgentChatQuickView(activeRoomId).catch(() => null);
        setQuickView(quick);
        return;
      }

      const report = await approveAutoCleanupProposalFromChat(proposal.root_id, proposal.proposal);
      if (activeRoomId) {
        suppressedOpenProposalRoomIdsRef.current.add(activeRoomId);
        setOpenProposals([]);
      }
      const executed = report.execution.executed_count;
      const skipped = report.execution.skipped_count;
      const skippedOnly = isLocalSkippedOnly(report.execution);
      if (skippedOnly) {
        setSkippedLocalProposalKeys((current) => new Set(current).add(localKey));
      }
      pushLocalStatusMessage(
        executionStatusMessage({
          roomId: activeRoomId ?? "",
          sessionId: chatSession?.session_id ?? null,
          content: formatLocalExecutionResult("자동 제안 실행 결과", report.execution)
        })
      );
      setNotice(`자동 제안 실행 완료: 실행 ${executed}개, 건너뜀 ${skipped}개`);
      if (activeRoomId) {
        const quick = await getAgentChatQuickView(activeRoomId).catch(() => null);
        setQuickView(quick);
      }
    } catch (cause) {
      setNotice(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
      setApprovingLocalProposalKeys((current) => {
        const next = new Set(current);
        next.delete(localKey);
        return next;
      });
    }
  }

  function beginChatWindowDrag(pointer: PointerEvent<HTMLElement>) {
    if (pointer.button !== 0) return;
    const target = pointer.target as HTMLElement;
    if (target.closest("button, input, textarea, select, a")) return;
    void getCurrentWindow().startDragging().catch((cause) => {
      console.error("Failed to drag chat overlay", cause);
    });
  }

  return (
    <div className="chat-overlay">
      <RoomChatPanel
        avatarUrl={chatAvatarUrl}
        answerPending={answerPending}
        busy={busy}
        draft={draft}
        event={event}
        notice={notice}
        openProposals={visibleOpenProposals}
        localAutoProposals={visibleLocalAutoProposals}
        roomId={activeRoomId}
        roomName={activeRoomName}
        messages={displayedMessages}
        loading={chatLoading}
        sessionTitle={chatSession?.title ?? null}
        quickView={quickView}
        approvedDraftIds={approvedDraftIds}
        approvingDraftIds={approvingDraftIds}
        approvingProposalIds={approvingProposalIds}
        approvingLocalProposalKeys={approvingLocalProposalKeys}
        submittedDraftIds={submittedDraftIds}
        submittedProposalIds={submittedProposalIds}
        submittedLocalProposalKeys={submittedLocalProposalKeys}
        skippedDraftIds={skippedDraftIds}
        skippedProposalIds={skippedProposalIds}
        skippedLocalProposalKeys={skippedLocalProposalKeys}
        onBeginWindowDrag={beginChatWindowDrag}
        onApproveCommandDraft={(draftId) => void approveCommandDraft(draftId)}
        onApproveOpenProposal={(proposalId) => void approveOpenProposal(proposalId)}
        onApproveLocalAutoProposal={(localKey) => void approveLocalAutoProposal(localKey)}
        onChangeDraft={setDraft}
        onClose={() => {
          void hideChatOverlay();
        }}
        onQuickCleanup={() => void runQuickCleanup()}
        onUsePrompt={setDraft}
        onSubmit={() => void submitDraft()}
      />
    </div>
  );
}

async function ensureRoomChatSession(roomId: string, roomName: string) {
  const sessions = await listAgentChatSessions(roomId);
  return sessions[0] ?? createAgentChatSession(roomId, `${roomName} chat`);
}

function appendUniqueMessages(current: AgentChatMessage[], next: AgentChatMessage[]) {
  const seen = new Set(current.map((message) => message.message_id));
  const merged = [...current];
  for (const message of next) {
    if (seen.has(message.message_id)) continue;
    seen.add(message.message_id);
    merged.push(message);
  }
  return merged;
}

function mergeMessagesByTime(serverMessages: AgentChatMessage[], localMessages: AgentChatMessage[]) {
  return appendUniqueMessages(serverMessages, localMessages).sort(
    (left, right) => Date.parse(left.created_at) - Date.parse(right.created_at)
  );
}

function buildChatTimeline(
  messages: AgentChatMessage[],
  openProposals: TimedOpenProposal[],
  localAutoProposals: LocalAutoProposal[]
): ChatTimelineItem[] {
  return [
    ...messages.map((message): ChatTimelineItem => ({
      kind: "message",
      key: `message:${message.message_id}`,
      timestamp: Date.parse(message.created_at),
      message
    })),
    ...openProposals.map((proposal): ChatTimelineItem => ({
      kind: "openProposal",
      key: `open:${proposal.proposal_id}`,
      timestamp: proposal.receivedAt,
      proposal
    })),
    ...localAutoProposals.map((proposal): ChatTimelineItem => ({
      kind: "localProposal",
      key: `local:${proposal.localKey}`,
      timestamp: proposal.receivedAt,
      proposal
    }))
  ].sort((left, right) => {
    const leftTime = Number.isFinite(left.timestamp) ? left.timestamp : 0;
    const rightTime = Number.isFinite(right.timestamp) ? right.timestamp : 0;
    return leftTime - rightTime || left.key.localeCompare(right.key);
  });
}

async function singleOpenProposalForRoom(roomId: string, fallback: AgentOpenProposal[]) {
  try {
    const open = (await listAgentOpenProposals(roomId)).filter(
      (proposal) => proposal.status === "OPEN"
    );
    return open.length === 1 ? open[0] : null;
  } catch {
    const open = fallback.filter((proposal) => proposal.room_id === roomId && proposal.status === "OPEN");
    return open.length === 1 ? open[0] : null;
  }
}

function executionStatusMessage({
  content,
  roomId,
  sessionId
}: {
  content: string;
  roomId: string;
  sessionId: string | null;
}): AgentChatMessage {
  return {
    message_id: `local-exec-${globalThis.crypto?.randomUUID?.() ?? Date.now()}`,
    room_id: roomId,
    session_id: sessionId,
    sender_type: "ASSISTANT",
    message_type: "EXECUTION_RESULT",
    content,
    structured_payload: null,
    command_id: null,
    created_at: new Date().toISOString()
  };
}

function lastAgentChatMessageId(messages: AgentChatMessage[]) {
  return messages.length > 0 ? messages[messages.length - 1].message_id : null;
}

function isChatMessage(message: AgentChatMessage | null): message is AgentChatMessage {
  return message != null;
}

function commandDraftApprovalKey(draftId: string) {
  return `chatdraft-${draftId.replace(/[^A-Za-z0-9_-]/g, "_").slice(0, 80)}`;
}

function proposalApprovalKey(proposalId: string) {
  return `chatproposal-${proposalId.replace(/[^A-Za-z0-9_-]/g, "_").slice(0, 80)}`;
}

function localAutoProposalKey(event: AutoCleanupProposalEvent) {
  const items = event.proposal.proposals
    .map((item) => `${item.proposal_id}:${item.from}:${item.to}:${item.action}`)
    .join("|");
  return `${event.root_id}:${items}`;
}

function proposalActionLabel(action: string) {
  if (action === "trash") return "버리기";
  if (action === "move") return "이동";
  if (action === "create_dir") return "폴더";
  if (action === "create_file") return "파일";
  if (action === "readme_write") return "README";
  return action;
}

function proposalPathLabel(path: string) {
  const normalized = path.replace(/\\/g, "/");
  const parts = normalized.split("/").filter(Boolean);
  return parts.length > 0 ? parts[parts.length - 1] : normalized;
}

function proposalDestinationLabel(action: string, to: string) {
  if (action === "trash") return "휴지통으로";
  if (!to.trim()) return "";
  return `→ ${to}`;
}

function formatDecisionExecutionResult(title: string, report: DecisionProcessingReport) {
  const failed = report.failed_count > 0 ? `, 실패 ${report.failed_count}개` : "";
  return `${title}\n실행 ${report.executed_item_count}개, 건너뜀 ${report.skipped_item_count}개${failed}`;
}

function isDecisionSkippedOnly(report: DecisionProcessingReport) {
  return report.executed_item_count === 0 && report.skipped_item_count > 0 && report.failed_count === 0;
}

function formatLocalExecutionResult(title: string, report: ExecuteReport) {
  const summary = `${title}\n실행 ${report.executed_count}개, 건너뜀 ${report.skipped_count}개, 거절 ${report.rejected_count}개`;
  const details = report.results
    .slice(0, 4)
    .map((result) => {
      const target = result.status === "executed" ? proposalDestinationLabel(result.action, result.to) : result.reason ?? "";
      return `- ${executionStatusLabel(result.status)}: ${proposalPathLabel(result.from)} ${target}`.trim();
    });
  const more = report.results.length > details.length ? [`- 외 ${report.results.length - details.length}개 더`] : [];
  return [summary, ...details, ...more].join("\n");
}

function isLocalSkippedOnly(report: ExecuteReport) {
  return report.executed_count === 0 && report.skipped_count > 0 && report.rejected_count === 0;
}

function executionStatusLabel(status: string) {
  if (status === "executed") return "완료";
  if (status === "skipped") return "건너뜀";
  if (status === "rejected") return "거절";
  return status;
}

function messageClassName(message: AgentChatMessage) {
  return [
    "room-chat-message",
    message.sender_type === "USER" ? "is-user" : "is-mouse",
    message.message_type === "EXECUTION_RESULT" ? "is-execution-result" : ""
  ]
    .filter(Boolean)
    .join(" ");
}

function aiNotice(status: string) {
  if (status === "UNCONFIGURED") {
    return "메시지는 저장됐고, AI 응답은 OpenAI 설정이 끝나면 붙어요. 서버의 AI_PROVIDER/AI_API_KEY/AI_MODEL을 확인해 주세요.";
  }
  if (status === "INVALID") {
    return "메시지는 저장됐지만 AI 응답 형식이 검증을 통과하지 못했어요.";
  }
  return null;
}

type CommandDraftPayload = {
  id: string;
  status: string;
};

function commandDraftPayload(payload: unknown): CommandDraftPayload | null {
  if (!payload || typeof payload !== "object") return null;
  const record = payload as Record<string, unknown>;
  return typeof record.id === "string" && typeof record.status === "string"
    ? { id: record.id, status: record.status }
    : null;
}

function CommandDraftActions({
  approvedDraftIds,
  approvingDraftIds,
  busy,
  payload,
  roomId,
  skippedDraftIds,
  submittedDraftIds,
  onApprove
}: {
  approvedDraftIds: Set<string>;
  approvingDraftIds: Set<string>;
  busy: boolean;
  payload: unknown;
  roomId: string | null;
  skippedDraftIds: Set<string>;
  submittedDraftIds: Set<string>;
  onApprove: (draftId: string) => void;
}) {
  const draft = commandDraftPayload(payload);
  if (!draft) {
    return <small className="room-chat-draft-hint">명령 상태를 확인할 수 없어요.</small>;
  }
  if (skippedDraftIds.has(draft.id)) {
    return <small className="room-chat-draft-hint">스킵됨</small>;
  }
  if (approvedDraftIds.has(draft.id)) {
    return <small className="room-chat-draft-hint">승인 후 실행 요청을 완료했어요.</small>;
  }
  if (submittedDraftIds.has(draft.id)) {
    return <small className="room-chat-draft-hint">실행 요청을 보냈어요.</small>;
  }
  if (draft.status !== "DRAFT") {
    return <small className="room-chat-draft-hint">명령 상태: {draft.status}</small>;
  }
  const approving = approvingDraftIds.has(draft.id);
  return (
    <button
      type="button"
      className="room-chat-draft-approve"
      onClick={() => onApprove(draft.id)}
      disabled={busy || approving || roomId == null}
    >
      {approving ? "실행 중" : "승인하고 실행"}
    </button>
  );
}

function OpenProposalActions({
  approvingProposalIds,
  busy,
  proposal,
  roomId,
  skippedProposalIds,
  submittedProposalIds,
  onApprove
}: {
  approvingProposalIds: Set<string>;
  busy: boolean;
  proposal: AgentOpenProposal;
  roomId: string | null;
  skippedProposalIds: Set<string>;
  submittedProposalIds: Set<string>;
  onApprove: (proposalId: string) => void;
}) {
  const approving = approvingProposalIds.has(proposal.proposal_id);
  const submitted = submittedProposalIds.has(proposal.proposal_id);
  const skipped = skippedProposalIds.has(proposal.proposal_id);
  return (
    <button
      type="button"
      className="room-chat-draft-approve"
      onClick={() => onApprove(proposal.proposal_id)}
      disabled={busy || approving || submitted || skipped || roomId == null}
    >
      {skipped ? "스킵됨" : approving ? "실행 중" : submitted ? "실행 요청됨" : "승인하고 실행"}
    </button>
  );
}

function LocalAutoProposalActions({
  approvingLocalProposalKeys,
  busy,
  proposal,
  skippedLocalProposalKeys,
  submittedLocalProposalKeys,
  onApprove
}: {
  approvingLocalProposalKeys: Set<string>;
  busy: boolean;
  proposal: LocalAutoProposal;
  skippedLocalProposalKeys: Set<string>;
  submittedLocalProposalKeys: Set<string>;
  onApprove: (localKey: string) => void;
}) {
  const approving = approvingLocalProposalKeys.has(proposal.localKey);
  const submitted = submittedLocalProposalKeys.has(proposal.localKey);
  const skipped = skippedLocalProposalKeys.has(proposal.localKey);
  return (
    <button
      type="button"
      className="room-chat-draft-approve"
      onClick={() => onApprove(proposal.localKey)}
      disabled={busy || approving || submitted || skipped}
    >
      {skipped ? "스킵됨" : approving ? "실행 중" : submitted ? "실행 요청됨" : "승인하고 실행"}
    </button>
  );
}

function RoomChatPanel({
  avatarUrl,
  answerPending,
  approvedDraftIds,
  approvingDraftIds,
  approvingLocalProposalKeys,
  approvingProposalIds,
  busy,
  draft,
  event,
  loading,
  localAutoProposals,
  messages,
  notice,
  openProposals,
  quickView,
  roomId,
  roomName,
  sessionTitle,
  skippedDraftIds,
  skippedLocalProposalKeys,
  skippedProposalIds,
  submittedDraftIds,
  submittedLocalProposalKeys,
  submittedProposalIds,
  onChangeDraft,
  onClose,
  onQuickCleanup,
  onBeginWindowDrag,
  onApproveCommandDraft,
  onApproveLocalAutoProposal,
  onApproveOpenProposal,
  onUsePrompt,
  onSubmit
}: {
  avatarUrl: string;
  answerPending: boolean;
  approvedDraftIds: Set<string>;
  approvingDraftIds: Set<string>;
  approvingLocalProposalKeys: Set<string>;
  approvingProposalIds: Set<string>;
  busy: boolean;
  draft: string;
  event: CharacterEvent;
  loading: boolean;
  localAutoProposals: LocalAutoProposal[];
  messages: AgentChatMessage[];
  notice: string | null;
  openProposals: TimedOpenProposal[];
  quickView: AgentChatQuickView | null;
  roomId: string | null;
  roomName: string;
  sessionTitle: string | null;
  skippedDraftIds: Set<string>;
  skippedLocalProposalKeys: Set<string>;
  skippedProposalIds: Set<string>;
  submittedDraftIds: Set<string>;
  submittedLocalProposalKeys: Set<string>;
  submittedProposalIds: Set<string>;
  onChangeDraft: (value: string) => void;
  onClose: () => void;
  onQuickCleanup: () => void;
  onBeginWindowDrag: (event: PointerEvent<HTMLElement>) => void;
  onApproveCommandDraft: (draftId: string) => void;
  onApproveLocalAutoProposal: (localKey: string) => void;
  onApproveOpenProposal: (proposalId: string) => void;
  onUsePrompt: (value: string) => void;
  onSubmit: () => void;
}) {
  const prompts = quickView?.prompts.length ? quickView.prompts.slice(0, 4) : fallbackQuickPrompts;
  const pendingCount = quickView?.pending_action_count ?? 0;
  const unreadCount = quickView?.unread_count ?? 0;
  const threadRef = useRef<HTMLDivElement | null>(null);
  const timelineItems = buildChatTimeline(messages, openProposals, localAutoProposals);

  useEffect(() => {
    const thread = threadRef.current;
    if (!thread) return;
    thread.scrollTop = thread.scrollHeight;
  }, [answerPending, loading, notice, timelineItems.length]);

  return (
    <div className="room-chat-panel" data-room-id={roomId ?? "unbound"}>
      <header className="room-chat-header" onPointerDown={onBeginWindowDrag}>
        <button type="button" onClick={onClose} aria-label="Close chat">
          &lt;
        </button>
        <div>
          <strong>{roomName}</strong>
          <small>{sessionTitle ?? "서버 채팅 세션 연결 중"}</small>
        </div>
        <span className="room-chat-status" title={event.kind} aria-label={event.kind} />
      </header>

      <div className="room-chat-quickbar" aria-label="Chat quick actions">
        <button type="button" onClick={onQuickCleanup} disabled={busy || roomId == null}>
          빠른 정리
        </button>
        {prompts.map((prompt) => (
          <button key={prompt.id} type="button" onClick={() => onUsePrompt(prompt.prompt)} title={prompt.prompt}>
            {prompt.label}
          </button>
        ))}
      </div>

      {pendingCount > 0 || unreadCount > 0 ? (
        <div className="room-chat-quickmeta" aria-label="Chat quick counts">
          {pendingCount > 0 ? <span>제안 {pendingCount}</span> : null}
          {unreadCount > 0 ? <span>새 메시지 {unreadCount}</span> : null}
        </div>
      ) : null}

      <div ref={threadRef} className="room-chat-thread" aria-live="polite">
        {loading ? (
          <article className="room-chat-message is-mouse">
            <img src={avatarUrl} alt="" aria-hidden="true" draggable={false} />
            <div>
              <strong>MouseKeeper</strong>
              <p>모바일과 공유하는 채팅 기록을 불러오는 중이에요.</p>
            </div>
          </article>
        ) : null}
        {!loading && timelineItems.length === 0 ? (
          <article className="room-chat-message is-mouse">
            <img src={avatarUrl} alt="" aria-hidden="true" draggable={false} />
            <div>
              <strong>MouseKeeper</strong>
              <p>{event.message ?? "이 방 기준으로 이야기할게요. 이 기록은 모바일 채팅과 같은 서버 세션에 저장돼요."}</p>
            </div>
          </article>
        ) : null}
        {timelineItems.map((item) => {
          if (item.kind === "message") {
            const message = item.message;
            return (
              <article
                className={messageClassName(message)}
                key={item.key}
              >
                {message.sender_type === "ASSISTANT" ? (
                  <img src={avatarUrl} alt="" aria-hidden="true" draggable={false} />
                ) : null}
                <div>
                  {message.sender_type === "ASSISTANT" ? <strong>MouseKeeper</strong> : null}
                  <p>{message.content}</p>
                  {message.message_type === "COMMAND_DRAFT" ? (
                    <div className="room-chat-draft-actions">
                      <small className="room-chat-draft-hint">승인 대기 중인 명령 초안이에요.</small>
                      <CommandDraftActions
                        payload={message.structured_payload}
                        roomId={roomId}
                        busy={busy}
                        approvedDraftIds={approvedDraftIds}
                        approvingDraftIds={approvingDraftIds}
                        skippedDraftIds={skippedDraftIds}
                        submittedDraftIds={submittedDraftIds}
                        onApprove={onApproveCommandDraft}
                      />
                    </div>
                  ) : null}
                </div>
              </article>
            );
          }

          if (item.kind === "openProposal") {
            const proposal = item.proposal;
            return (
              <article className="room-chat-message is-mouse is-auto-proposal" key={item.key}>
                <img src={avatarUrl} alt="" aria-hidden="true" draggable={false} />
                <div>
                  <strong>MouseKeeper</strong>
                  <p>정리할 만한 항목을 찾았어요. 승인하면 바로 확인하고 실행할게요.</p>
                  <div className="room-chat-draft-actions">
                    <small className="room-chat-draft-hint">자동 제안 승인 대기 중</small>
                    <OpenProposalActions
                      proposal={proposal}
                      roomId={roomId}
                      busy={busy}
                      approvingProposalIds={approvingProposalIds}
                      skippedProposalIds={skippedProposalIds}
                      submittedProposalIds={submittedProposalIds}
                      onApprove={onApproveOpenProposal}
                    />
                  </div>
                </div>
              </article>
            );
          }

          const proposal = item.proposal;
          return (
            <article className="room-chat-message is-mouse is-auto-proposal" key={item.key}>
              <img src={avatarUrl} alt="" aria-hidden="true" draggable={false} />
              <div>
                <strong>MouseKeeper</strong>
                <p>이렇게 바꾸면 좋을 것 같은데요, 찍?</p>
                <ul className="room-chat-proposal-list">
                  {proposal.proposal.proposals.slice(0, 5).map((proposalItem) => (
                    <li key={proposalItem.proposal_id}>
                      <span>{proposalActionLabel(proposalItem.action)}</span>
                      <strong>{proposalPathLabel(proposalItem.from)}</strong>
                      <small>{proposalDestinationLabel(proposalItem.action, proposalItem.to)}</small>
                    </li>
                  ))}
                  {proposal.proposal.proposals.length > 5 ? (
                    <li className="is-more">외 {proposal.proposal.proposals.length - 5}개 더</li>
                  ) : null}
                </ul>
                <div className="room-chat-draft-actions">
                  <small className="room-chat-draft-hint">
                    승인하면 사전 점검 후 바로 실행할게요.
                  </small>
                  <LocalAutoProposalActions
                    proposal={proposal}
                    busy={busy}
                    approvingLocalProposalKeys={approvingLocalProposalKeys}
                    skippedLocalProposalKeys={skippedLocalProposalKeys}
                    submittedLocalProposalKeys={submittedLocalProposalKeys}
                    onApprove={onApproveLocalAutoProposal}
                  />
                </div>
              </div>
            </article>
          );
        })}
      </div>

      {answerPending ? <p className="character-thinking">답을 만들고 있어요!</p> : null}

      {notice ? <p className="character-notice">{notice}</p> : null}

      <form
        className="character-chat"
        onSubmit={(submitEvent) => {
          submitEvent.preventDefault();
          onSubmit();
        }}
      >
        <input
          aria-label="Send message draft"
          placeholder={`${roomName}에 메시지 입력`}
          maxLength={2000}
          value={draft}
          onChange={(inputEvent) => onChangeDraft(inputEvent.target.value)}
          autoFocus
        />
        <button type="submit" disabled={busy || draft.trim().length === 0}>
          전송
        </button>
      </form>
    </div>
  );
}
