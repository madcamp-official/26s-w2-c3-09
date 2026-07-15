import { useEffect, useRef, useState } from "react";
import type { MouseEvent, PointerEvent } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { AgentPanel, AutostartSetting } from "../agent/AgentPanel";
import { FileEnginePanel } from "../files/FileEnginePanel";
import {
  getLatestCleanlinessSnapshot,
  listManagedRoots,
  ManagedRoot,
  listenForManagedRootBindingChanges,
  listenForCleanlinessSnapshotUpdates
} from "../files/fileEngineApi";
import {
  HOUSE_DROP_TARGET_EVENT,
  hideChatOverlay,
  hideOverlay,
  publishChatRoomSelection,
  setHouseOverlayLocked
} from "./overlayApi";

const houseUrls = [
  new URL("../../../../../packages/character-assets/house/mouse_house1.png", import.meta.url).href,
  new URL("../../../../../packages/character-assets/house/mouse_house2.png", import.meta.url).href,
  new URL("../../../../../packages/character-assets/house/mouse_house3.png", import.meta.url).href,
  new URL("../../../../../packages/character-assets/house/mouse_house4.png", import.meta.url).href,
  new URL("../../../../../packages/character-assets/house/mouse_house5.png", import.meta.url).href
] as const;
const messLayerUrls = {
  floor: new URL(
    "../../../../../packages/character-assets/mess/mess_floor_dirt_transparent.png",
    import.meta.url
  ).href,
  wall: new URL(
    "../../../../../packages/character-assets/mess/mess_wall_stains_transparent.png",
    import.meta.url
  ).href,
  mess: new URL("../../assets/mess-dirty-transparent.png", import.meta.url).href
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
  const [roots, setRoots] = useState<ManagedRoot[]>([]);
  const [selectedRootId, setSelectedRootId] = useState("");
  const activeRootIdRef = useRef<string | null>(null);
  const rootsRef = useRef<ManagedRoot[]>([]);
  const dragStart = useRef<DragStart | null>(null);
  const draggingStarted = useRef(false);
  const suppressHouseClick = useRef(false);
  const clickTimer = useRef<number | null>(null);

  function selectHouseRoot(rootId: string) {
    const nextRootId = rootId || null;
    activeRootIdRef.current = nextRootId;
    setSelectedRootId(rootId);
    void publishChatRoomSelection(nextRootId);
  }

  async function refreshHouseRootsForMenu() {
    const storedRoots = await listManagedRoots();
    const visibleRoots =
      storedRoots.length === 0 && rootsRef.current.length > 0 && activeRootIdRef.current
        ? rootsRef.current
        : storedRoots;
    rootsRef.current = visibleRoots;
    setRoots(visibleRoots);
    const root = selectStableRoot(visibleRoots, activeRootIdRef.current);
    if (root) selectHouseRoot(root.root_id);
    return root;
  }

  useEffect(() => {
    return () => {
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
        const root = await refreshHouseRootsForMenu();
        if (stopped) return;
        if (!root) {
          return;
        }
        const snapshot = await getLatestCleanlinessSnapshot(root.root_id);
        if (!stopped && activeRootIdRef.current === root.root_id) {
          setCleanlinessScore(snapshot?.score ?? null);
        }
      } catch {
        // Keep the last known score during transient store/bootstrap errors so the house does not
        // flash clean/empty while the background runtime is reconciling roots.
      }
    }

    void refreshCleanliness();
    const unlisten = listenForCleanlinessSnapshotUpdates((update) => {
      if (!stopped && update.rootId === activeRootIdRef.current) {
        setCleanlinessScore(update.snapshot.score);
      }
    });
    let unlistenBinding: (() => void) | undefined;
    void listenForManagedRootBindingChanges(() => {
      void refreshCleanliness();
    }).then((off) => {
      unlistenBinding = off;
    });
    return () => {
      stopped = true;
      void unlisten.then((off) => off());
      unlistenBinding?.();
    };
  }, []);

  useEffect(() => {
    if (!window.__TAURI_INTERNALS__) return;
    if (!activeSection) return;
    const selectedStillExists = roots.some((root) => root.root_id === selectedRootId);
    if (selectedStillExists) return;
    const preferred = selectStableRoot(roots, selectedRootId);
    if (preferred) {
      selectHouseRoot(preferred.root_id);
    }
  }, [activeSection, roots, selectedRootId]);

  function beginHousePointer(pointer: PointerEvent<HTMLButtonElement>) {
    if (pointer.button !== 0) return;
    if (locked) return;
    dragStart.current = { x: pointer.clientX, y: pointer.clientY };
    draggingStarted.current = false;
    suppressHouseClick.current = false;
    pointer.currentTarget.setPointerCapture(pointer.pointerId);
  }

  function continueHouseDrag(pointer: PointerEvent<HTMLButtonElement>) {
    const start = dragStart.current;
    if (!start || draggingStarted.current) return;
    const moved = Math.hypot(pointer.clientX - start.x, pointer.clientY - start.y);
    if (moved < 4) return;

    draggingStarted.current = true;
    suppressHouseClick.current = true;
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
    if (suppressHouseClick.current) {
      suppressHouseClick.current = false;
      return;
    }
    void hideChatOverlay().catch(() => undefined);
    if (clickTimer.current !== null) {
      window.clearTimeout(clickTimer.current);
    }
    clickTimer.current = window.setTimeout(() => {
      void refreshHouseRootsForMenu().catch(() => undefined);
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
    void hideChatOverlay().catch(() => undefined);
    setMenuOpen(false);
    setActiveSection(section);
    void refreshHouseRootsForMenu().catch(() => undefined);
  }

  function closeOverlay(event: MouseEvent<HTMLButtonElement>) {
    event.stopPropagation();
    setMenuOpen(false);
    setActiveSection(null);
    void hideOverlay()
      .catch((cause) => {
        console.error("Failed to hide all overlays", cause);
      })
      .finally(() => {
        void getCurrentWindow().hide().catch(() => {
          // Browser preview has no Tauri runtime; the desktop shell hides the window.
        });
      });
  }

  function stopPanelPointer(event: MouseEvent<HTMLElement>) {
    event.stopPropagation();
  }

  const messLevel = messLevelForCleanliness(cleanlinessScore);
  const houseImage = houseUrlForRoot(roots, selectedRootId);

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
        <HouseAmbientMessLayers level={messLevel} />
        <img
          key={houseImage}
          className="house-overlay-image"
          src={houseImage}
          alt=""
          draggable={false}
        />
        <HouseMessProps level={messLevel} />
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
            {renderManagerSection(activeSection, selectedRootId, selectHouseRoot, roots)}
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

function HouseAmbientMessLayers({ level }: { level: number }) {
  if (level <= 0) return null;
  return (
    <div className={`house-mess-layers house-ambient-mess-layers mess-level-${level}`} aria-hidden="true">
      {level >= 1 ? <img src={messLayerUrls.floor} alt="" draggable={false} /> : null}
      {level >= 2 ? <img src={messLayerUrls.wall} alt="" draggable={false} /> : null}
    </div>
  );
}

function HouseMessProps({ level }: { level: number }) {
  if (level < 2) return null;
  return (
    <div className={`house-mess-props mess-level-${level}`} aria-hidden="true">
      <img src={messLayerUrls.mess} alt="" draggable={false} />
    </div>
  );
}

function selectStableRoot(roots: ManagedRoot[], currentRootId: string | null) {
  return (
    roots.find((root) => root.root_id === currentRootId) ??
    roots.find((root) => root.room_binding_status === "active") ??
    roots[0] ??
    null
  );
}

function houseUrlForRoot(roots: ManagedRoot[], rootId: string) {
  const index = rootOrderIndex(roots, rootId);
  return houseUrls[index % houseUrls.length];
}

function rootOrderIndex(roots: ManagedRoot[], rootId: string) {
  if (!rootId) return 0;
  const ordered = [...roots].sort((left, right) => {
    const registered = left.registered_unix_ms - right.registered_unix_ms;
    if (registered !== 0) return registered;
    return left.root_id.localeCompare(right.root_id);
  });
  const index = ordered.findIndex((root) => root.root_id === rootId);
  return index < 0 ? 0 : index;
}

function renderManagerSection(
  section: HousePanelSection,
  selectedRootId: string,
  setSelectedRootId: (rootId: string) => void,
  roots: ManagedRoot[]
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
          initialRoots={roots}
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
      initialRoots={roots}
    />
  );
}
