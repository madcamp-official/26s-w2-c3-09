import { useEffect, useMemo, useRef, useState } from "react";
import type { PointerEvent } from "react";
import { getCurrentWindow, LogicalPosition } from "@tauri-apps/api/window";

import { listManagedRoots } from "../files/fileEngineApi";
import type { ManagedRoot } from "../files/fileEngineApi";
import {
  CharacterEvent,
  CharacterEventKind,
  listenForCharacterEvents,
  submitOverlayDraftRequest
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

export function CharacterOverlay() {
  const [event, setEvent] = useState<CharacterEvent>({ kind: "IDLE" });
  const [draft, setDraft] = useState("");
  const [sentDrafts, setSentDrafts] = useState<string[]>([]);
  const [rooms, setRooms] = useState<ManagedRoot[]>([]);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [bubbleOpen, setBubbleOpen] = useState(false);
  const [chatOpen, setChatOpen] = useState(false);
  const [isDraggingOverlay, setIsDraggingOverlay] = useState(false);
  const [pointerHeld, setPointerHeld] = useState(false);
  const [dragReleaseTick, setDragReleaseTick] = useState(0);
  const dragStart = useRef<DragStart | null>(null);
  const dragOrigin = useRef<DragOrigin | null>(null);
  const draggingStarted = useRef(false);
  const suppressClick = useRef(false);

  const activeRoom = rooms.find((root) => root.room_binding_status === "active") ?? rooms[0] ?? null;
  const activeRoomName = activeRoom?.display_name ?? "방 없음";
  const activeRoomId = activeRoom?.room_id ?? activeRoom?.root_id ?? "unbound";

  useEffect(() => {
    document.documentElement.classList.add("overlay-root");
    document.body.classList.add("overlay-body");
    return () => {
      document.documentElement.classList.remove("overlay-root");
      document.body.classList.remove("overlay-body");
    };
  }, []);

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

  function releaseDrag() {
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
        .catch(() => {
          // Browser preview cannot move a native window; Tauri handles this in the desktop shell.
        });
    };
    window.addEventListener("pointermove", onPointerMoveGlobal);
    window.addEventListener("pointerup", releaseDrag);
    window.addEventListener("mouseup", releaseDrag);
    window.addEventListener("blur", releaseDrag);
    return () => {
      window.removeEventListener("pointermove", onPointerMoveGlobal);
      window.removeEventListener("pointerup", releaseDrag);
      window.removeEventListener("mouseup", releaseDrag);
      window.removeEventListener("blur", releaseDrag);
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
    releaseDrag();
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
      await submitOverlayDraftRequest(trimmed);
      setSentDrafts((items) => [...items.slice(-3), trimmed]);
      setDraft("");
      setNotice("이 방의 초안으로 보냈어요.");
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
              sentDrafts={sentDrafts}
              onChangeDraft={setDraft}
              onClose={() => {
                setChatOpen(false);
                setNotice(null);
              }}
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

function RoomChatPanel({
  avatarUrl,
  busy,
  draft,
  event,
  notice,
  roomId,
  roomName,
  sentDrafts,
  onChangeDraft,
  onClose,
  onSubmit
}: {
  avatarUrl: string;
  busy: boolean;
  draft: string;
  event: CharacterEvent;
  notice: string | null;
  roomId: string;
  roomName: string;
  sentDrafts: string[];
  onChangeDraft: (value: string) => void;
  onClose: () => void;
  onSubmit: () => void;
}) {
  return (
    <div className="room-chat-panel" data-room-id={roomId}>
      <header className="room-chat-header">
        <button type="button" onClick={onClose} aria-label="Close chat">
          &lt;
        </button>
        <div>
          <strong>{roomName}</strong>
          <small>방별 채팅 준비 중</small>
        </div>
        <span className="room-chat-status" title={event.kind} aria-label={event.kind} />
      </header>

      <div className="room-chat-thread" aria-live="polite">
        <article className="room-chat-message is-mouse">
          <img src={avatarUrl} alt="" aria-hidden="true" draggable={false} />
          <div>
            <strong>MouseKeeper</strong>
            <p>{event.message ?? "이 방 기준으로 이야기할게요. 나중에는 방별 대화가 여기에 따로 쌓여요."}</p>
          </div>
        </article>
        <article className="room-chat-message is-mouse">
          <img src={avatarUrl} alt="" aria-hidden="true" draggable={false} />
          <div>
            <strong>MouseKeeper</strong>
            <p>집 이미지, 청결도, 채팅 스레드는 같은 roomId로 연결될 예정이에요.</p>
          </div>
        </article>
        {sentDrafts.map((item, index) => (
          <article className="room-chat-message is-user" key={`${index}-${item}`}>
            <p>{item}</p>
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
          전송
        </button>
      </form>
    </div>
  );
}
