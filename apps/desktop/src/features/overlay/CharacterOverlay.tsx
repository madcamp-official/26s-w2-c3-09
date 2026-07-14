import { useEffect, useMemo, useRef, useState } from "react";
import type { PointerEvent } from "react";
import { getCurrentWindow, LogicalPosition } from "@tauri-apps/api/window";

import {
  CharacterEvent,
  CharacterEventKind,
  listenForCharacterEvents,
  submitOverlayDraftRequest
} from "./overlayApi";
import { useCharacterWander } from "./characterWander";

// Animations played while the mouse roams the desktop on its own. Paths are kept as static string
// literals so Vite can resolve the assets at build time.
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

// Shown while the user is dragging the mouse around by the cursor.
const danglingMotionUrl = new URL(
  "../../../../../packages/character-assets/new_mouse/gif/mouse_dangling_preview.gif",
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
  // Cursor position at grab time, in screen (CSS) pixels — stable while the window itself moves.
  x: number;
  y: number;
  at: number;
};

type DragOrigin = {
  // Window's outer position at grab time, in logical (CSS) pixels — same space as pointer.screenX.
  winX: number;
  winY: number;
};

export function CharacterOverlay() {
  const [event, setEvent] = useState<CharacterEvent>({ kind: "IDLE" });
  const [draft, setDraft] = useState("");
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [bubbleOpen, setBubbleOpen] = useState(false);
  const [chatOpen, setChatOpen] = useState(false);
  const [isDraggingOverlay, setIsDraggingOverlay] = useState(false);
  const [pointerHeld, setPointerHeld] = useState(false);
  // Bumped each time an actual drag (movement past the threshold, not just a click) ends, so the
  // wander hook can tell "user dropped it here" apart from other reasons roaming paused.
  const [dragReleaseTick, setDragReleaseTick] = useState(0);
  const dragStart = useRef<DragStart | null>(null);
  const dragOrigin = useRef<DragOrigin | null>(null);
  const draggingStarted = useRef(false);
  const suppressClick = useRef(false);

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

  // Shared by the button's own pointerup/cancel handler and the window-level fallback below.
  // Whichever fires first (they both run for a normal release, since the button's handler runs
  // before the event bubbles to `window`) reports the drag and clears the flag, so the other is a
  // harmless no-op — this avoids a race where the button's handler clears `draggingStarted` before
  // the window listener gets a chance to see it was a real drag.
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

  // Drag tracking lives on `window` rather than the drag-surface button's own pointer capture.
  // The button visually moves out from under the cursor every time we reposition the native
  // window mid-drag, and some WebView engines drop pointer capture when that happens; a
  // window-level listener keeps receiving move/up events regardless of which element the browser
  // currently thinks is "under" the pointer.
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
      // pointerEvent.screenX/Y and LogicalPosition are both in CSS pixels, so the cursor delta
      // maps straight onto the window position with no DPI conversion.
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

  // The mouse only roams on its own when it is idle and the user is not touching it. Any real
  // MouseKeeper activity (connecting, analyzing, working, …) parks it so its state animation reads
  // clearly at a fixed spot.
  const wanderActive =
    event.kind === "IDLE" && !isDraggingOverlay && !pointerHeld && !bubbleOpen;
  const { phase: wanderPhase, facing } = useCharacterWander(wanderActive, dragReleaseTick);

  const motionUrl = useMemo(() => {
    if (isDraggingOverlay) return danglingMotionUrl;
    if (wanderActive) return wanderMotionUrls[wanderPhase];
    return overlayMotionUrls[event.kind];
  }, [event.kind, isDraggingOverlay, wanderActive, wanderPhase]);

  // Sprites are authored facing left; mirror horizontally when the mouse walks to the right so it
  // always faces the direction it is travelling.
  const spriteFlip = wanderActive && facing === 1 ? "scaleX(-1)" : "none";

  function beginOverlayDrag(pointer: PointerEvent<HTMLButtonElement>) {
    if (pointer.button !== 0) return;
    dragStart.current = { x: pointer.screenX, y: pointer.screenY, at: Date.now() };
    dragOrigin.current = null;
    draggingStarted.current = false;
    suppressClick.current = false;
    // Freeze roaming the moment the mouse is grabbed so it can be clicked or dragged cleanly.
    setPointerHeld(true);
    pointer.currentTarget.setPointerCapture(pointer.pointerId);
    // Record where the window starts so pointer deltas can move it directly. We drive the drag
    // ourselves (rather than the native startDragging move-loop) so the dangling animation keeps
    // rendering and updating for the whole drag.
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
    setNotice(null);
    setBusy(true);
    try {
      await submitOverlayDraftRequest(draft);
      setDraft("");
      setChatOpen(false);
      setBubbleOpen(false);
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
        aria-label={`MouseKeeper ${KIND_LABELS[event.kind]}. 클릭하면 말풍선을 열고, 드래그하면 위치를 옮깁니다.`}
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
        <section className={`character-bubble ${chatOpen ? "is-chatting" : ""}`} aria-label="MouseKeeper 말풍선">
          {chatOpen ? (
            <form
              className="character-chat"
              onSubmit={(e) => {
                e.preventDefault();
                void submitDraft();
              }}
            >
              <input
                aria-label="정리 제안 요청하기"
                placeholder="정리 요청"
                maxLength={2000}
                value={draft}
                onChange={(e) => setDraft(e.target.value)}
                autoFocus
              />
              <button type="submit" disabled={busy || draft.trim().length === 0}>
                전송
              </button>
            </form>
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
          {notice ? <p className="character-notice">{notice}</p> : null}
        </section>
      ) : null}
    </div>
  );
}
