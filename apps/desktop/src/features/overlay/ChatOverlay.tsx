import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { listManagedRoots } from "../files/fileEngineApi";
import type { ManagedRoot } from "../files/fileEngineApi";
import {
  createAgentChatQuickCleanup,
  createAgentChatSession,
  getAgentChatQuickView,
  listAgentChatMessages,
  listAgentChatSessions,
  markAgentChatSessionRead,
  sendAgentChatMessage
} from "../agent/agentApi";
import type { AgentChatMessage, AgentChatQuickView, AgentChatSession } from "../agent/agentApi";
import {
  CharacterEvent,
  hideChatOverlay,
  listenForCharacterEvents
} from "./overlayApi";

const chatAvatarUrl = new URL(
  "../../../../../packages/character-assets/new_mouse/mouse_walk_clean.png",
  import.meta.url
).href;

/**
 * The chat lives in its own fixed-size window (`chat-overlay`) instead of resizing the mascot
 * window. It loads its own room context, so the mascot window only has to show/hide it.
 */
export function ChatOverlay() {
  const [event, setEvent] = useState<CharacterEvent>({ kind: "IDLE" });
  const [rooms, setRooms] = useState<ManagedRoot[]>([]);
  const [chatSession, setChatSession] = useState<AgentChatSession | null>(null);
  const [quickView, setQuickView] = useState<AgentChatQuickView | null>(null);
  const [chatMessages, setChatMessages] = useState<AgentChatMessage[]>([]);
  const [draft, setDraft] = useState("");
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [answerPending, setAnswerPending] = useState(false);
  const [chatLoading, setChatLoading] = useState(false);

  const activeRoom = rooms.find((root) => root.room_binding_status === "active") ?? rooms[0] ?? null;
  const activeRoomName = activeRoom?.display_name ?? "방 없음";
  const activeRoomId = activeRoom?.room_id ?? null;

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

  useEffect(() => {
    const unlisten = listenForCharacterEvents(setEvent);
    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

  useEffect(() => {
    void loadRooms();
  }, []);

  useEffect(() => {
    if (!window.__TAURI_INTERNALS__) return;
    const unlisten = getCurrentWindow().onFocusChanged((focusEvent) => {
      if (focusEvent.payload) {
        void loadRooms();
        return;
      }
      void hideChatOverlay().catch(() => undefined);
    });
    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

  useEffect(() => {
    if (!activeRoomId) {
      setChatSession(null);
      setQuickView(null);
      setChatMessages([]);
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

  return (
    <div className="chat-overlay">
      <RoomChatPanel
        avatarUrl={chatAvatarUrl}
        answerPending={answerPending}
        busy={busy}
        draft={draft}
        event={event}
        notice={notice}
        roomId={activeRoomId}
        roomName={activeRoomName}
        messages={chatMessages}
        loading={chatLoading}
        sessionTitle={chatSession?.title ?? null}
        quickView={quickView}
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

function lastAgentChatMessageId(messages: AgentChatMessage[]) {
  return messages.length > 0 ? messages[messages.length - 1].message_id : null;
}

function isChatMessage(message: AgentChatMessage | null): message is AgentChatMessage {
  return message != null;
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

function RoomChatPanel({
  avatarUrl,
  answerPending,
  busy,
  draft,
  event,
  loading,
  messages,
  notice,
  quickView,
  roomId,
  roomName,
  sessionTitle,
  onChangeDraft,
  onClose,
  onQuickCleanup,
  onUsePrompt,
  onSubmit
}: {
  avatarUrl: string;
  answerPending: boolean;
  busy: boolean;
  draft: string;
  event: CharacterEvent;
  loading: boolean;
  messages: AgentChatMessage[];
  notice: string | null;
  quickView: AgentChatQuickView | null;
  roomId: string | null;
  roomName: string;
  sessionTitle: string | null;
  onChangeDraft: (value: string) => void;
  onClose: () => void;
  onQuickCleanup: () => void;
  onUsePrompt: (value: string) => void;
  onSubmit: () => void;
}) {
  const prompts = quickView?.prompts.slice(0, 4) ?? [];
  const pendingCount = quickView?.pending_action_count ?? 0;
  const unreadCount = quickView?.unread_count ?? 0;
  return (
    <div className="room-chat-panel" data-room-id={roomId ?? "unbound"}>
      <header className="room-chat-header">
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

      <div className="room-chat-thread" aria-live="polite">
        {loading ? (
          <article className="room-chat-message is-mouse">
            <img src={avatarUrl} alt="" aria-hidden="true" draggable={false} />
            <div>
              <strong>MouseKeeper</strong>
              <p>모바일과 공유하는 채팅 기록을 불러오는 중이에요.</p>
            </div>
          </article>
        ) : null}
        {!loading && messages.length === 0 ? (
          <article className="room-chat-message is-mouse">
            <img src={avatarUrl} alt="" aria-hidden="true" draggable={false} />
            <div>
              <strong>MouseKeeper</strong>
              <p>{event.message ?? "이 방 기준으로 이야기할게요. 이 기록은 모바일 채팅과 같은 서버 세션에 저장돼요."}</p>
            </div>
          </article>
        ) : null}
        {messages.map((message) => (
          <article
            className={`room-chat-message ${message.sender_type === "USER" ? "is-user" : "is-mouse"}`}
            key={message.message_id}
          >
            {message.sender_type === "ASSISTANT" ? (
              <img src={avatarUrl} alt="" aria-hidden="true" draggable={false} />
            ) : null}
            <div>
              {message.sender_type === "ASSISTANT" ? <strong>MouseKeeper</strong> : null}
              <p>{message.content}</p>
              {message.message_type === "COMMAND_DRAFT" ? (
                <small className="room-chat-draft-hint">모바일에서 승인할 수 있는 명령 초안이에요.</small>
              ) : null}
            </div>
          </article>
        ))}
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
