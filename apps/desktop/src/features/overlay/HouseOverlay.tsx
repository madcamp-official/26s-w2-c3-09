import { useEffect, useRef } from "react";
import type { PointerEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { setHouseOverlayLocked } from "./overlayApi";

const houseUrl = new URL("../../assets/mouse-house-transparent.png", import.meta.url).href;

type DragStart = {
  x: number;
  y: number;
};

export function HouseOverlay() {
  const dragStart = useRef<DragStart | null>(null);
  const draggingStarted = useRef(false);

  useEffect(() => {
    document.documentElement.classList.add("overlay-root");
    document.body.classList.add("house-overlay-body");
    return () => {
      document.documentElement.classList.remove("overlay-root");
      document.body.classList.remove("house-overlay-body");
    };
  }, []);

  function beginHousePointer(pointer: PointerEvent<HTMLButtonElement>) {
    if (pointer.button !== 0) return;
    dragStart.current = { x: pointer.clientX, y: pointer.clientY };
    draggingStarted.current = false;
    pointer.currentTarget.setPointerCapture(pointer.pointerId);
  }

  function continueHouseDrag(pointer: PointerEvent<HTMLButtonElement>) {
    const start = dragStart.current;
    if (!start || draggingStarted.current) return;
    const moved = Math.hypot(pointer.clientX - start.x, pointer.clientY - start.y);
    if (moved < 4) return;

    draggingStarted.current = true;
    void getCurrentWindow()
      .startDragging()
      .catch(() => {
        // Browser preview cannot move a native window; Tauri handles this in the desktop shell.
      });
  }

  function finishHousePointer(pointer: PointerEvent<HTMLButtonElement>) {
    dragStart.current = null;
    draggingStarted.current = false;
    if (pointer.currentTarget.hasPointerCapture(pointer.pointerId)) {
      pointer.currentTarget.releasePointerCapture(pointer.pointerId);
    }
  }

  function lockHouse() {
    void setHouseOverlayLocked(true);
  }

  return (
    <div className="house-overlay">
      <button
        type="button"
        className="house-drag-surface"
        onDoubleClick={lockHouse}
        onPointerDown={beginHousePointer}
        onPointerMove={continueHouseDrag}
        onPointerUp={finishHousePointer}
        onPointerCancel={finishHousePointer}
        aria-label="집 오버레이 이동"
        title="드래그: 집 위치 이동 / 더블클릭: 위치 고정"
      >
        <img className="house-overlay-image" src={houseUrl} alt="" draggable={false} />
      </button>
    </div>
  );
}
