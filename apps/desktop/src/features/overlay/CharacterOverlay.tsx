import { useEffect, useMemo, useRef, useState } from "react";
import type { PointerEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import {
  CharacterEvent,
  CharacterEventKind,
  listenForCharacterEvents,
  submitOverlayDraftRequest
} from "./overlayApi";

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
  "../../assets/mouse-dangling-transparent.png",
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

export function CharacterOverlay() {
  const [event, setEvent] = useState<CharacterEvent>({ kind: "IDLE" });
  const [draft, setDraft] = useState("");
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [bubbleOpen, setBubbleOpen] = useState(false);
  const [chatOpen, setChatOpen] = useState(false);
  const [isDraggingOverlay, setIsDraggingOverlay] = useState(false);
  const dragStart = useRef<DragStart | null>(null);
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

  useEffect(() => {
    const stopDragging = () => {
      dragStart.current = null;
      draggingStarted.current = false;
      setIsDraggingOverlay(false);
    };
    window.addEventListener("pointerup", stopDragging);
    window.addEventListener("mouseup", stopDragging);
    window.addEventListener("blur", stopDragging);
    return () => {
      window.removeEventListener("pointerup", stopDragging);
      window.removeEventListener("mouseup", stopDragging);
      window.removeEventListener("blur", stopDragging);
    };
  }, []);

  const motionUrl = useMemo(
    () => (isDraggingOverlay ? danglingMotionUrl : overlayMotionUrls[event.kind]),
    [event.kind, isDraggingOverlay]
  );

  function beginOverlayDrag(pointer: PointerEvent<HTMLButtonElement>) {
    if (pointer.button !== 0) return;
    dragStart.current = { x: pointer.clientX, y: pointer.clientY, at: Date.now() };
    draggingStarted.current = false;
    suppressClick.current = false;
    pointer.currentTarget.setPointerCapture(pointer.pointerId);
  }

  function continueOverlayDrag(pointer: PointerEvent<HTMLButtonElement>) {
    const start = dragStart.current;
    if (!start || draggingStarted.current) return;
    const moved = Math.hypot(pointer.clientX - start.x, pointer.clientY - start.y);
    if (moved < 4) return;

    draggingStarted.current = true;
    suppressClick.current = true;
    setIsDraggingOverlay(true);
    void getCurrentWindow()
      .startDragging()
      .catch(() => {
        // Browser preview cannot move a native window; Tauri handles this in the desktop shell.
      })
      .finally(() => {
        dragStart.current = null;
        draggingStarted.current = false;
        setIsDraggingOverlay(false);
      });
  }

  function finishOverlayPointer(pointer: PointerEvent<HTMLButtonElement>) {
    const start = dragStart.current;
    dragStart.current = null;
    draggingStarted.current = false;
    setIsDraggingOverlay(false);
    if (pointer.currentTarget.hasPointerCapture(pointer.pointerId)) {
      pointer.currentTarget.releasePointerCapture(pointer.pointerId);
    }
    if (!start) return;
    const moved = Math.hypot(pointer.clientX - start.x, pointer.clientY - start.y);
    suppressClick.current = moved > 5 || Date.now() - start.at > 350;
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
        onPointerMove={continueOverlayDrag}
        onPointerUp={finishOverlayPointer}
        onPointerCancel={finishOverlayPointer}
        aria-label={`MouseKeeper ${KIND_LABELS[event.kind]}. 클릭하면 말풍선을 열고, 드래그하면 위치를 옮깁니다.`}
        title="클릭: 말풍선 / 드래그: 이동"
      >
        <img className="character-motion" src={motionUrl} alt="" aria-hidden="true" draggable={false} />
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
