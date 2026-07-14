import { useEffect, useMemo, useRef, useState } from "react";
import type { PointerEvent } from "react";
import { emit } from "@tauri-apps/api/event";
import {
  getAllWindows,
  getCurrentWindow,
  LogicalPosition,
  LogicalSize,
  PhysicalPosition
} from "@tauri-apps/api/window";

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
  CharacterEventKind,
  HOUSE_DROP_TARGET_EVENT,
  HOUSE_OVERLAY_WINDOW_LABEL,
  listenForCharacterEvents,
} from "./overlayApi";
import { useCharacterWander } from "./characterWander";

const wanderMotionUrls = {
  walking: new URL(
    "../../../../../packages/character-assets/new_mouse/gif/mouse_walk_preview.gif",
    import.meta.url
  ).href,
  pausing: new URL(
    "../../../../../packages/character-assets/new_mouse/gif/mouse_idle_preview.gif",
    import.meta.url
  ).href,
  resting: new URL(
    "../../../../../packages/character-assets/new_mouse/gif/mouse_sleep_preview.gif",
    import.meta.url
  ).href
} as const;

const overlayMotionUrls: Record<CharacterEventKind, string> = {
  IDLE: new URL("../../../../../packages/character-assets/new_mouse/gif/mouse_idle_preview.gif", import.meta.url).href,
  CONNECTING: new URL("../../../../../packages/character-assets/new_mouse/gif/mouse_walk_preview.gif", import.meta.url).href,
  ANALYZING: new URL("../../../../../packages/character-assets/new_mouse/gif/mouse_work_preview.gif", import.meta.url).href,
  WAITING_APPROVAL: new URL(
    "../../../../../packages/character-assets/new_mouse/gif/mouse_organize_preview.gif",
    import.meta.url
  ).href,
  WORKING: new URL("../../../../../packages/character-assets/new_mouse/gif/mouse_work_preview.gif", import.meta.url).href,
  SUCCESS: new URL("../../../../../packages/character-assets/new_mouse/gif/mouse_organize_preview.gif", import.meta.url).href,
  ERROR: new URL("../../../../../packages/character-assets/new_mouse/gif/mouse_idle_preview.gif", import.meta.url).href,
  USER_WORKING: new URL("../../../../../packages/character-assets/new_mouse/gif/mouse_walk_preview.gif", import.meta.url).href,
  OFFLINE: new URL("../../../../../packages/character-assets/new_mouse/gif/mouse_idle_preview.gif", import.meta.url).href
};

const danglingMotionUrl = new URL(
  "../../../../../packages/character-assets/new_mouse/gif/mouse_dangling_preview.gif",
  import.meta.url
).href;
const chatAvatarUrl = new URL(
  "../../../../../packages/character-assets/new_mouse/mouse_walk_clean.png",
  import.meta.url
).href;

const KIND_LABELS: Record<CharacterEventKind, string> = {
  IDLE: "대기 중",
  CONNECTING: "연결 중",
  ANALYZING: "분석 중",
  WAITING_APPROVAL: "승인 대기",
  WORKING: "작업 중",
  SUCCESS: "완료",
  ERROR: "확인 필요",
  USER_WORKING: "작업 중",
  OFFLINE: "오프라인"
};

type DragStart = {
  x: number;
  y: number;
  at: number;
};

type DragOrigin = {
  winX: number;
  winY: number;
};

type Point = { x: number; y: number };
type Size = { w: number; h: number };
type Rect = { x: number; y: number; w: number; h: number };
type Polygon = readonly Point[];
type HouseDropTarget = { active: boolean };

const HOUSE_FOOT_OFFSET = { x: 0.5, y: 0.82 } as const;
const OVERLAY_SIZES = {
  closed: { width: 112, height: 140 },
  prompt: { width: 190, height: 140 },
  chat: { width: 450, height: 360 }
} as const;

export function CharacterOverlay() {
  const [event, setEvent] = useState<CharacterEvent>({ kind: "IDLE" });
  const [draft, setDraft] = useState("");
  const [rooms, setRooms] = useState<ManagedRoot[]>([]);
  const [chatSession, setChatSession] = useState<AgentChatSession | null>(null);
  const [quickView, setQuickView] = useState<AgentChatQuickView | null>(null);
  const [chatMessages, setChatMessages] = useState<AgentChatMessage[]>([]);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [chatLoading, setChatLoading] = useState(false);
  const [bubbleOpen, setBubbleOpen] = useState(false);
  const [chatOpen, setChatOpen] = useState(false);
  const [isDraggingOverlay, setIsDraggingOverlay] = useState(false);
  const [pointerHeld, setPointerHeld] = useState(false);
  const [dragReleaseTick, setDragReleaseTick] = useState(0);
  const dragStart = useRef<DragStart | null>(null);
  const dragOrigin = useRef<DragOrigin | null>(null);
  const draggingStarted = useRef(false);
  const suppressClick = useRef(false);
  const houseDropActive = useRef(false);

  const activeRoom = rooms.find((root) => root.room_binding_status === "active") ?? rooms[0] ?? null;
  const activeRoomName = activeRoom?.display_name ?? "방 없음";
  const activeRoomId = activeRoom?.room_id ?? null;

  useEffect(() => {
    document.documentElement.classList.add("overlay-root");
    document.body.classList.add("overlay-body");
    return () => {
      document.documentElement.classList.remove("overlay-root");
      document.body.classList.remove("overlay-body");
    };
  }, []);

  useEffect(() => {
    const mode = chatOpen ? "chat" : bubbleOpen ? "prompt" : "closed";
    void resizeOverlayWindow(mode);
  }, [bubbleOpen, chatOpen]);

  useEffect(() => {
    const unlisten = listenForCharacterEvents(setEvent);
    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

  useEffect(() => {
    if (!bubbleOpen) return;
    void listManagedRoots()
      .then(setRooms)
      .catch(() => setRooms([]));
  }, [bubbleOpen]);

  useEffect(() => {
    if (!chatOpen) return;
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
  }, [activeRoomId, activeRoomName, chatOpen]);

  useEffect(() => {
    if (!chatOpen || !chatSession) return;
    const timer = window.setInterval(() => {
      void listAgentChatMessages(chatSession.session_id)
        .then(async (messages) => {
          await markAgentChatSessionRead(chatSession.session_id, lastAgentChatMessageId(messages)).catch(() => undefined);
          setChatMessages(messages);
        })
        .catch(() => undefined);
    }, 5000);
    return () => window.clearInterval(timer);
  }, [chatOpen, chatSession]);

  async function setHouseDropTarget(active: boolean) {
    if (houseDropActive.current === active) return;
    houseDropActive.current = active;
    if (!window.__TAURI_INTERNALS__) return;
    try {
      await emit(HOUSE_DROP_TARGET_EVENT, { active } satisfies HouseDropTarget);
    } catch {
      /* the house overlay may not be open */
    }
  }

  async function releaseDrag() {
    const shouldSnapHome =
      draggingStarted.current && (houseDropActive.current || (await isCurrentWindowOverHouse()));
    if (shouldSnapHome) {
      await snapCharacterToHouseFloor();
    }
    await setHouseDropTarget(false);
    if (draggingStarted.current) {
      setDragReleaseTick((tick) => tick + 1);
    }
    dragStart.current = null;
    dragOrigin.current = null;
    draggingStarted.current = false;
    setIsDraggingOverlay(false);
    setPointerHeld(false);
  }

  useEffect(() => {
    const onPointerMoveGlobal = (pointerEvent: globalThis.PointerEvent) => {
      const start = dragStart.current;
      if (!start) return;
      const moved = Math.hypot(pointerEvent.screenX - start.x, pointerEvent.screenY - start.y);
      if (!draggingStarted.current) {
        if (moved < 4) return;
        draggingStarted.current = true;
        suppressClick.current = true;
        setIsDraggingOverlay(true);
      }

      const origin = dragOrigin.current;
      if (!origin) return;
      const nextX = origin.winX + (pointerEvent.screenX - start.x);
      const nextY = origin.winY + (pointerEvent.screenY - start.y);
      void getCurrentWindow()
        .setPosition(new LogicalPosition(nextX, nextY))
        .then(() => isCurrentWindowOverHouse())
        .then((active) => setHouseDropTarget(active))
        .catch(() => {
          // Browser preview cannot move a native window; Tauri handles this in the desktop shell.
        });
    };
    const onReleaseGlobal = () => void releaseDrag();
    window.addEventListener("pointermove", onPointerMoveGlobal);
    window.addEventListener("pointerup", onReleaseGlobal);
    window.addEventListener("mouseup", onReleaseGlobal);
    window.addEventListener("blur", onReleaseGlobal);
    return () => {
      window.removeEventListener("pointermove", onPointerMoveGlobal);
      window.removeEventListener("pointerup", onReleaseGlobal);
      window.removeEventListener("mouseup", onReleaseGlobal);
      window.removeEventListener("blur", onReleaseGlobal);
    };
  }, []);

  const wanderActive = event.kind === "IDLE" && !isDraggingOverlay && !pointerHeld && !bubbleOpen;
  const { phase: wanderPhase, facing } = useCharacterWander(wanderActive, dragReleaseTick);

  const motionUrl = useMemo(() => {
    if (isDraggingOverlay) return danglingMotionUrl;
    if (wanderActive) return wanderMotionUrls[wanderPhase];
    return overlayMotionUrls[event.kind];
  }, [event.kind, isDraggingOverlay, wanderActive, wanderPhase]);

  const spriteFlip = wanderActive && facing === 1 ? "scaleX(-1)" : "none";

  function beginOverlayDrag(pointer: PointerEvent<HTMLButtonElement>) {
    if (pointer.button !== 0) return;
    dragStart.current = { x: pointer.screenX, y: pointer.screenY, at: Date.now() };
    dragOrigin.current = null;
    draggingStarted.current = false;
    suppressClick.current = false;
    setPointerHeld(true);
    pointer.currentTarget.setPointerCapture(pointer.pointerId);
    if (window.__TAURI_INTERNALS__) {
      const win = getCurrentWindow();
      void Promise.all([win.outerPosition(), win.scaleFactor()])
        .then(([position, scale]) => {
          const logical = position.toLogical(scale);
          dragOrigin.current = { winX: logical.x, winY: logical.y };
        })
        .catch(() => {
          /* no window position available; drag will simply show the dangling pose in place */
        });
    }
  }

  function finishOverlayPointer(pointer: PointerEvent<HTMLButtonElement>) {
    const start = dragStart.current;
    if (pointer.currentTarget.hasPointerCapture(pointer.pointerId)) {
      pointer.currentTarget.releasePointerCapture(pointer.pointerId);
    }
    if (start) {
      const moved = Math.hypot(pointer.screenX - start.x, pointer.screenY - start.y);
      suppressClick.current = moved > 5 || Date.now() - start.at > 350;
    }
    void releaseDrag();
  }

  function openBubble() {
    if (suppressClick.current) {
      suppressClick.current = false;
      return;
    }
    setBubbleOpen((open) => {
      if (open) {
        setChatOpen(false);
        setNotice(null);
        return false;
      }
      setChatOpen(false);
      setNotice(null);
      return true;
    });
  }

  async function submitDraft() {
    const trimmed = draft.trim();
    setNotice(null);
    setBusy(true);
    try {
      if (!activeRoomId) {
        throw new Error("먼저 관리 폴더를 서버 room과 연결해야 채팅을 보낼 수 있어요.");
      }
      const session = chatSession ?? (await ensureRoomChatSession(activeRoomId, activeRoomName));
      if (!chatSession) setChatSession(session);
      const result = await sendAgentChatMessage(session.session_id, trimmed);
      const sentMessages = [result.message, result.assistant].filter(isChatMessage);
      await markAgentChatSessionRead(session.session_id, lastAgentChatMessageId(sentMessages)).catch(() => undefined);
      setChatMessages((items) => appendUniqueMessages(items, sentMessages));
      setDraft("");
      setNotice(aiNotice(result.ai_status));
    } catch (cause) {
      setNotice(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
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
    <div className={`character-overlay character-${event.kind.toLowerCase()}`}>
      <button
        type="button"
        className="character-drag-surface"
        onClick={openBubble}
        onPointerDown={beginOverlayDrag}
        onPointerUp={finishOverlayPointer}
        onPointerCancel={finishOverlayPointer}
        aria-label={`MouseKeeper ${KIND_LABELS[event.kind]}`}
        title="클릭: 말풍선 / 드래그: 이동"
      >
        <img
          className="character-motion"
          src={motionUrl}
          alt=""
          aria-hidden="true"
          draggable={false}
          style={{ transform: spriteFlip }}
        />
      </button>

      {bubbleOpen ? (
        <section className={`character-bubble ${chatOpen ? "is-chatting" : ""}`} aria-label="MouseKeeper chat">
          {chatOpen ? (
            <RoomChatPanel
              avatarUrl={chatAvatarUrl}
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
                setChatOpen(false);
                setNotice(null);
              }}
              onQuickCleanup={() => void runQuickCleanup()}
              onUsePrompt={setDraft}
              onSubmit={() => void submitDraft()}
            />
          ) : (
            <button
              type="button"
              className="character-bubble-prompt"
              onClick={() => {
                setChatOpen(true);
                setNotice(null);
              }}
              aria-label="채팅창 열기"
            >
              <span className="character-bubble-dots" aria-hidden="true">
                <span />
                <span />
                <span />
              </span>
            </button>
          )}
        </section>
      ) : null}
    </div>
  );
}

async function readHouseRect(): Promise<Rect | null> {
  try {
    const house = (await getAllWindows()).find(
      (candidate) => candidate.label === HOUSE_OVERLAY_WINDOW_LABEL
    );
    if (!house) return null;
    const [position, size] = await Promise.all([house.outerPosition(), house.outerSize()]);
    const houseSide = Math.min(size.width, size.height);
    const houseX = position.x + Math.max(0, size.width - houseSide);
    const houseY = position.y + Math.max(0, size.height - houseSide);
    return { x: houseX, y: houseY, w: houseSide, h: houseSide };
  } catch {
    return null;
  }
}

async function resizeOverlayWindow(mode: keyof typeof OVERLAY_SIZES) {
  if (!window.__TAURI_INTERNALS__) return;
  const win = getCurrentWindow();
  const target = OVERLAY_SIZES[mode];
  try {
    const [position, before] = await Promise.all([win.outerPosition(), win.outerSize()]);
    await win.setSize(new LogicalSize(target.width, target.height));
    const after = await win.outerSize();
    // The mascot is pinned to the bottom-right corner. Moving by the size delta keeps its feet
    // at the same desktop coordinate while the speech bubble opens or closes.
    await win.setPosition(
      new PhysicalPosition(
        position.x + before.width - after.width,
        position.y + before.height - after.height
      )
    );
  } catch {
    // Browser preview and a closing native window do not expose a usable window handle.
  }
}

async function readCharacterSize(): Promise<Size> {
  try {
    const size = await getCurrentWindow().outerSize();
    return { w: size.width, h: size.height };
  } catch {
    return { w: 184, h: 160 };
  }
}

function footPoint(pos: Point, size: Size) {
  return {
    x: pos.x + size.w * HOUSE_FOOT_OFFSET.x,
    y: pos.y + size.h * HOUSE_FOOT_OFFSET.y
  };
}

function topLeftFromFoot(point: Point, size: Size) {
  return {
    x: point.x - size.w * HOUSE_FOOT_OFFSET.x,
    y: point.y - size.h * HOUSE_FOOT_OFFSET.y
  };
}

function houseFloorPolygon(house: Rect): Polygon {
  const x = (ratio: number) => house.x + house.w * ratio;
  const y = (ratio: number) => house.y + house.h * ratio;
  return [
    { x: x(0.1), y: y(0.72) },
    { x: x(0.37), y: y(0.56) },
    { x: x(0.72), y: y(0.59) },
    { x: x(0.9), y: y(0.72) },
    { x: x(0.55), y: y(0.95) },
    { x: x(0.1), y: y(0.79) }
  ];
}

function pointInPolygon(point: Point, polygon: Polygon) {
  let inside = false;
  for (let i = 0, j = polygon.length - 1; i < polygon.length; j = i, i += 1) {
    const a = polygon[i];
    const b = polygon[j];
    const crosses =
      a.y > point.y !== b.y > point.y &&
      point.x < ((b.x - a.x) * (point.y - a.y)) / (b.y - a.y || Number.EPSILON) + a.x;
    if (crosses) inside = !inside;
  }
  return inside;
}

function closestFloorPoint(point: Point, polygon: Polygon) {
  if (pointInPolygon(point, polygon)) return point;

  const xs = polygon.map((vertex) => vertex.x);
  const ys = polygon.map((vertex) => vertex.y);
  const minX = Math.min(...xs);
  const maxX = Math.max(...xs);
  const minY = Math.min(...ys);
  const maxY = Math.max(...ys);
  let best = { x: (minX + maxX) / 2, y: (minY + maxY) / 2 };
  let bestDistance = Number.POSITIVE_INFINITY;

  for (let x = minX; x <= maxX; x += 12) {
    for (let y = minY; y <= maxY; y += 12) {
      const candidate = { x, y };
      if (!pointInPolygon(candidate, polygon)) continue;
      const distance = Math.hypot(candidate.x - point.x, candidate.y - point.y);
      if (distance < bestDistance) {
        best = candidate;
        bestDistance = distance;
      }
    }
  }

  return best;
}

function centerInside(pos: Point, size: Size, rect: Rect) {
  const center = { x: pos.x + size.w / 2, y: pos.y + size.h / 2 };
  return center.x >= rect.x && center.x <= rect.x + rect.w && center.y >= rect.y && center.y <= rect.y + rect.h;
}

async function isCurrentWindowOverHouse() {
  if (!window.__TAURI_INTERNALS__) return false;
  const [house, size, position] = await Promise.all([
    readHouseRect(),
    readCharacterSize(),
    getCurrentWindow().outerPosition().catch(() => null)
  ]);
  return !!house && !!position && centerInside({ x: position.x, y: position.y }, size, house);
}

async function snapCharacterToHouseFloor() {
  const [house, size, position] = await Promise.all([
    readHouseRect(),
    readCharacterSize(),
    getCurrentWindow().outerPosition().catch(() => null)
  ]);
  if (!house || !position) return;

  const current = { x: position.x, y: position.y };
  const targetFoot = closestFloorPoint(footPoint(current, size), houseFloorPolygon(house));
  const next = topLeftFromFoot(targetFoot, size);
  await getCurrentWindow()
    .setPosition(new PhysicalPosition(Math.round(next.x), Math.round(next.y)))
    .catch(() => undefined);
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
          {busy ? "전송 중" : "전송"}
        </button>
      </form>
    </div>
  );
}
