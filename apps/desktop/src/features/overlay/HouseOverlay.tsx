import { useEffect, useRef, useState } from "react";
import type { MouseEvent, PointerEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { AgentPanel, AutostartSetting } from "../agent/AgentPanel";
import { FileEnginePanel } from "../files/FileEnginePanel";
import { setHouseOverlayLocked } from "./overlayApi";

const houseUrl = new URL("../../assets/mouse-house-transparent.png", import.meta.url).href;

type DragStart = {
  x: number;
  y: number;
};

type HousePanelSection = "rooms" | "organize" | "explore" | "history" | "connection" | "settings";
type FileSection = Exclude<HousePanelSection, "connection">;

const HOUSE_PANEL_ITEMS: ReadonlyArray<{
  id: HousePanelSection;
  label: string;
  hint: string;
}> = [
  { id: "rooms", label: "방 관리", hint: "등록, 연결 해제" },
  { id: "organize", label: "폴더 정리", hint: "제안, 승인, 실행" },
  { id: "explore", label: "폴더 탐색", hint: "파일 조회, 검색" },
  { id: "history", label: "작업 기록", hint: "결과, 되돌리기" },
  { id: "connection", label: "PC 연결", hint: "페어링, 동기화" },
  { id: "settings", label: "설정", hint: "감시, 자동 실행" }
];

export function HouseOverlay() {
  const [locked, setLocked] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const [activeSection, setActiveSection] = useState<HousePanelSection | null>(null);
  const dragStart = useRef<DragStart | null>(null);
  const draggingStarted = useRef(false);
  const clickTimer = useRef<number | null>(null);

  useEffect(() => {
    document.documentElement.classList.add("overlay-root");
    document.body.classList.add("house-overlay-body");
    return () => {
      document.documentElement.classList.remove("overlay-root");
      document.body.classList.remove("house-overlay-body");
      if (clickTimer.current !== null) {
        window.clearTimeout(clickTimer.current);
      }
    };
  }, []);

  function beginHousePointer(pointer: PointerEvent<HTMLButtonElement>) {
    if (pointer.button !== 0) return;
    if (locked) return;
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

  function queueHouseClick() {
    if (!locked) return;
    if (clickTimer.current !== null) {
      window.clearTimeout(clickTimer.current);
    }
    clickTimer.current = window.setTimeout(() => {
      setMenuOpen((open) => !open);
      setActiveSection(null);
      clickTimer.current = null;
    }, 180);
  }

  function toggleHouseLock() {
    if (clickTimer.current !== null) {
      window.clearTimeout(clickTimer.current);
      clickTimer.current = null;
    }
    setLocked((current) => {
      const next = !current;
      if (!next) {
        setMenuOpen(false);
        setActiveSection(null);
      }
      void setHouseOverlayLocked(next);
      return next;
    });
  }

  function openSection(section: HousePanelSection) {
    setActiveSection(section);
    setMenuOpen(false);
  }

  function stopPanelPointer(event: MouseEvent<HTMLElement>) {
    event.stopPropagation();
  }

  return (
    <div className={`house-overlay ${locked ? "is-locked" : ""}`}>
      <button
        type="button"
        className="house-drag-surface"
        onClick={queueHouseClick}
        onDoubleClick={toggleHouseLock}
        onPointerDown={beginHousePointer}
        onPointerMove={continueHouseDrag}
        onPointerUp={finishHousePointer}
        onPointerCancel={finishHousePointer}
        aria-label={locked ? "MouseKeeper house locked" : "MouseKeeper house draggable"}
        title={locked ? "Click to open menu / double-click to unlock" : "Drag to move / double-click to lock"}
      >
        <img className="house-overlay-image" src={houseUrl} alt="" draggable={false} />
      </button>

      {menuOpen ? (
        <nav className="house-quick-menu" aria-label="MouseKeeper house menu" onClick={stopPanelPointer}>
          {HOUSE_PANEL_ITEMS.map((item) => (
            <button
              key={item.id}
              type="button"
              onClick={(event) => {
                event.stopPropagation();
                openSection(item.id);
              }}
            >
              <strong>{item.label}</strong>
              <small>{item.hint}</small>
            </button>
          ))}
        </nav>
      ) : null}

      {activeSection ? (
        <section
          className="house-detail-panel house-manager-panel"
          aria-label="MouseKeeper compact manager"
          onClick={stopPanelPointer}
          onMouseDown={stopPanelPointer}
          onPointerDown={(event) => event.stopPropagation()}
        >
          <header>
            <button type="button" onClick={() => setActiveSection(null)} aria-label="Back to house menu">
              &lt;
            </button>
            <div>
              <strong>{HOUSE_PANEL_ITEMS.find((item) => item.id === activeSection)?.label}</strong>
              <small>Manager compact view</small>
            </div>
            <button type="button" onClick={() => setMenuOpen(true)} aria-label="Open menu">
              =
            </button>
          </header>
          <div className="house-manager-content">{renderManagerSection(activeSection)}</div>
        </section>
      ) : null}
    </div>
  );
}

function renderManagerSection(section: HousePanelSection) {
  if (section === "connection") {
    return <AgentPanel showAutostart={false} />;
  }

  if (section === "settings") {
    return (
      <>
        <section className="panel house-autostart-card">
          <div className="section-header">
            <div>
              <h2>PC 실행 설정</h2>
              <p className="path-text">자동 실행과 파일 관리 정책을 조정합니다.</p>
            </div>
          </div>
          <AutostartSetting />
        </section>
        <FileEnginePanel embedded activeSection="settings" />
      </>
    );
  }

  return <FileEnginePanel embedded activeSection={section as FileSection} />;
}
