import { useEffect, useRef, useState } from "react";
import type { MouseEvent, PointerEvent } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { AgentPanel, AutostartSetting } from "../agent/AgentPanel";
import { FileEnginePanel } from "../files/FileEnginePanel";
import {
  getLatestCleanlinessSnapshot,
  listManagedRoots,
  listenForCleanlinessSnapshotUpdates
} from "../files/fileEngineApi";
import { HOUSE_DROP_TARGET_EVENT, hideOverlay, setHouseOverlayLocked } from "./overlayApi";

const houseUrl = new URL("../../assets/mouse-house-transparent.png", import.meta.url).href;
const messLayerUrls = {
  floor: new URL("../../../../../packages/character-assets/mess/floor_dirty.png", import.meta.url).href,
  wall: new URL("../../../../../packages/character-assets/mess/wall_dirty.png", import.meta.url).href,
  mess: new URL("../../../../../packages/character-assets/mess/mess_dirty.png", import.meta.url).href,
  web: new URL("../../../../../packages/character-assets/mess/web_dirty.png", import.meta.url).href
} as const;

type DragStart = {
  x: number;
  y: number;
};

type HousePanelSection = "rooms" | "organize" | "explore" | "history" | "connection" | "settings";
type FileSection = Exclude<HousePanelSection, "connection" | "explore">;

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
  const [dropTarget, setDropTarget] = useState(false);
  const [cleanlinessScore, setCleanlinessScore] = useState<number | null>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  const [activeSection, setActiveSection] = useState<HousePanelSection | null>(null);
  const [selectedRootId, setSelectedRootId] = useState("");
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

  useEffect(() => {
    if (!window.__TAURI_INTERNALS__) return;
    const unlisten = listen<{ active: boolean }>(HOUSE_DROP_TARGET_EVENT, (event) => {
      setDropTarget(event.payload.active);
    });
    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

  useEffect(() => {
    if (!window.__TAURI_INTERNALS__) return;
    let stopped = false;

    async function refreshCleanliness() {
      try {
        const roots = await listManagedRoots();
        const root = roots.find((item) => item.room_binding_status === "active") ?? roots[0];
        if (!root) {
          if (!stopped) setCleanlinessScore(null);
          return;
        }
        const snapshot = await getLatestCleanlinessSnapshot(root.root_id);
        if (!stopped) setCleanlinessScore(snapshot?.score ?? null);
      } catch {
        if (!stopped) setCleanlinessScore(null);
      }
    }

    void refreshCleanliness();
    const unlisten = listenForCleanlinessSnapshotUpdates((update) => {
      if (!stopped) setCleanlinessScore(update.snapshot.score);
    });
    return () => {
      stopped = true;
      void unlisten.then((off) => off());
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

  function closeOverlay(event: MouseEvent<HTMLButtonElement>) {
    event.stopPropagation();
    setMenuOpen(false);
    setActiveSection(null);
    void hideOverlay().catch(() => {
      // Browser preview has no Tauri runtime; the desktop shell hides the windows.
    });
  }

  function stopPanelPointer(event: MouseEvent<HTMLElement>) {
    event.stopPropagation();
  }

  return (
    <div className={`house-overlay ${locked ? "is-locked" : ""} ${dropTarget ? "is-drop-target" : ""}`}>
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
        <HouseMessLayers level={messLevelForCleanliness(cleanlinessScore)} />
      </button>

      {menuOpen ? (
        <nav className="house-quick-menu" aria-label="MouseKeeper house menu" onClick={stopPanelPointer}>
          {HOUSE_PANEL_ITEMS.filter((item) => item.id !== "explore").map((item) => (
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
          <button type="button" className="house-quick-menu-exit" onClick={closeOverlay}>
            <strong>오버레이 종료하기</strong>
            <small>화면에서 마우스와 집 숨기기</small>
          </button>
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
          <div className="house-manager-content">
            {renderManagerSection(activeSection, selectedRootId, setSelectedRootId)}
          </div>
        </section>
      ) : null}
    </div>
  );
}

function messLevelForCleanliness(score: number | null) {
  if (score === null || score >= 90) return 0;
  if (score >= 75) return 1;
  if (score >= 55) return 2;
  if (score >= 35) return 3;
  return 4;
}

function HouseMessLayers({ level }: { level: number }) {
  if (level <= 0) return null;
  return (
    <div className={`house-mess-layers mess-level-${level}`} aria-hidden="true">
      {level >= 1 ? <img src={messLayerUrls.floor} alt="" draggable={false} /> : null}
      {level >= 2 ? <img src={messLayerUrls.wall} alt="" draggable={false} /> : null}
      {level >= 2 ? (
        <img className="mess-highlight" src={messLayerUrls.mess} alt="" draggable={false} />
      ) : null}
      {level >= 4 ? <img src={messLayerUrls.web} alt="" draggable={false} /> : null}
    </div>
  );
}

function renderManagerSection(
  section: HousePanelSection,
  selectedRootId: string,
  setSelectedRootId: (rootId: string) => void
) {
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
        <FileEnginePanel
          embedded
          activeSection="settings"
          selectedRootId={selectedRootId}
          onSelectedRootIdChange={setSelectedRootId}
          hideRootPicker
        />
      </>
    );
  }

  return (
    <FileEnginePanel
      embedded
      activeSection={section as FileSection}
      selectedRootId={selectedRootId}
      onSelectedRootIdChange={setSelectedRootId}
      hideRootPicker={section !== "rooms"}
    />
  );
}
