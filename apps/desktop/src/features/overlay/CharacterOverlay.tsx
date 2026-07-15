import { useEffect, useMemo, useRef, useState } from "react";
import type { PointerEvent } from "react";
import { emit, listen } from "@tauri-apps/api/event";
import {
  getAllWindows,
  getCurrentWindow,
  PhysicalPosition
} from "@tauri-apps/api/window";

import {
  CHAT_OVERLAY_CLOSED_EVENT,
  CharacterEvent,
  CharacterEventKind,
  HOUSE_DROP_TARGET_EVENT,
  HOUSE_OVERLAY_WINDOW_LABEL,
  hideChatOverlay,
  hideSpeechBubble,
  listenForCharacterEvents,
  listenForChatRoomSelection,
  listenForSpeechBubbleClosed,
  publishChatAutoProposal,
  readChatRoomSelection,
  showChatOverlay,
  showSpeechBubble
} from "./overlayApi";
import { listenForAutoCleanupProposals } from "../files/fileEngineApi";
import type { AutoCleanupProposalEvent } from "../files/fileEngineApi";
import { getAgentChatQuickView } from "../agent/agentApi";
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
const angryMotionUrl = new URL(
  "../../../../../packages/character-assets/new_mouse/gif/mouse_mad_preview.gif",
  import.meta.url
).href;
const sighMotionUrl = new URL(
  "../../../../../packages/character-assets/new_mouse/gif/mouse_pathetic_preview.gif",
  import.meta.url
).href;
const KIND_LABELS: Record<CharacterEventKind, string> = {
  IDLE: "idle",
  CONNECTING: "connecting",
  ANALYZING: "analyzing",
  WAITING_APPROVAL: "waiting approval",
  WORKING: "working",
  SUCCESS: "complete",
  ERROR: "needs attention",
  USER_WORKING: "working",
  OFFLINE: "offline"
};

type DragStart = {
  x: number;
  y: number;
};

type DragMode = "idle" | "manual";

type DragOrigin = {
  pointerX: number;
  pointerY: number;
  windowX: number;
  windowY: number;
};

type Point = { x: number; y: number };
type Size = { w: number; h: number };
type Rect = { x: number; y: number; w: number; h: number };
type Polygon = readonly Point[];
type HouseDropTarget = { active: boolean };

const HOUSE_FOOT_OFFSET = { x: 0.5, y: 0.82 } as const;
const DRAG_THRESHOLD_PX = 10;
const TRANSIENT_EVENT_IDLE_DELAYS_MS: Partial<Record<CharacterEventKind, number>> = {
  CONNECTING: 8000,
  ANALYZING: 8000,
  WORKING: 8000,
  USER_WORKING: 8000,
  SUCCESS: 4500,
  ERROR: 6500
};

// How long the mouse tolerates being dragged around before it gets fed up, wriggles free, and
// gets mad for a bit.
const DRAG_TOO_LONG_MS = 15000;
const ANGRY_DURATION_MS = 2500;

// Chat "piling up" mood: polled independently of whether the chat window is open.
const CHAT_SIGH_POLL_MS = 20000;
const CHAT_SIGH_UNREAD_THRESHOLD = 3;

// Idle small-talk speech bubble: only fires while fully idle (see `wanderActive`).
const IDLE_SPEECH_MIN_DELAY_MS = 45000;
const IDLE_SPEECH_MAX_DELAY_MS = 90000;
const IDLE_SPEECH_LINES = ["뭐 해?", "나랑 놀자~", "심심해~", "재밌어?"] as const;

export function CharacterOverlay() {
  const [event, setEvent] = useState<CharacterEvent>({ kind: "IDLE" });
  const [chatOpen, setChatOpen] = useState(false);
  const [hasChatAttention, setHasChatAttention] = useState(false);
  const [isDraggingOverlay, setIsDraggingOverlay] = useState(false);
  const [pointerHeld, setPointerHeld] = useState(false);
  const [dragReleaseTick, setDragReleaseTick] = useState(0);
  const [angryUntil, setAngryUntil] = useState<number | null>(null);
  const [chatSighActive, setChatSighActive] = useState(false);
  const [speechActive, setSpeechActive] = useState(false);
  const dragStart = useRef<DragStart | null>(null);
  const lastDragPoint = useRef<DragStart | null>(null);
  const dragOrigin = useRef<DragOrigin | null>(null);
  const dragMode = useRef<DragMode>("idle");
  const draggingStarted = useRef(false);
  const suppressClick = useRef(false);
  const pointerStartedWithChatOpen = useRef(false);
  const houseDropActive = useRef(false);
  const latestAutoProposal = useRef<AutoCleanupProposalEvent | null>(null);
  const dragAngryTimer = useRef<number | null>(null);
  const speechTimer = useRef<number | null>(null);

  useEffect(() => {
    const unlisten = listenForCharacterEvents((nextEvent) => {
      setEvent(nextEvent);
      if (nextEvent.kind === "WAITING_APPROVAL") {
        setHasChatAttention(true);
      }
    });
    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

  useEffect(() => {
    const delay = TRANSIENT_EVENT_IDLE_DELAYS_MS[event.kind];
    if (!delay) return;
    const eventKey = `${event.kind}:${event.correlation_id ?? ""}:${event.message ?? ""}`;
    const timer = window.setTimeout(() => {
      setEvent((current) => {
        const currentKey = `${current.kind}:${current.correlation_id ?? ""}:${current.message ?? ""}`;
        return currentKey === eventKey ? { kind: "IDLE" } : current;
      });
    }, delay);
    return () => window.clearTimeout(timer);
  }, [event.kind, event.correlation_id, event.message]);

  // The angry reaction is a timed pulse, not a durable state: it clears itself once its window
  // passes, the same way TRANSIENT_EVENT_IDLE_DELAYS_MS reverts server-driven states.
  useEffect(() => {
    if (!angryUntil) return;
    const remaining = angryUntil - Date.now();
    const timer = window.setTimeout(
      () => setAngryUntil((current) => (current === angryUntil ? null : current)),
      Math.max(0, remaining)
    );
    return () => window.clearTimeout(timer);
  }, [angryUntil]);

  // Any window may hide chat (close button, house menu, drag start); keep wander/toggle in sync.
  useEffect(() => {
    const unlisten = listen(CHAT_OVERLAY_CLOSED_EVENT, () => setChatOpen(false));
    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

  useEffect(() => {
    const unlisten = listenForAutoCleanupProposals((proposal) => {
      if (proposal.proposal.proposals.length === 0) return;
      latestAutoProposal.current = proposal;
      setHasChatAttention(true);
      void publishChatAutoProposal(proposal);
    });
    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

  useEffect(() => {
    for (const src of [
      danglingMotionUrl,
      angryMotionUrl,
      sighMotionUrl,
      wanderMotionUrls.walking,
      wanderMotionUrls.resting
    ]) {
      const image = new Image();
      image.src = src;
    }
  }, []);

  // Chat "piling up" mood: polled independently of the chat window's own lifecycle so the sigh
  // still shows up while chat has never been opened this session.
  useEffect(() => {
    let cancelled = false;
    let roomId = readChatRoomSelection().rootId;

    async function refreshChatSigh() {
      if (!roomId) {
        if (!cancelled) setChatSighActive(false);
        return;
      }
      const quick = await getAgentChatQuickView(roomId).catch(() => null);
      if (cancelled) return;
      setChatSighActive((quick?.unread_count ?? 0) >= CHAT_SIGH_UNREAD_THRESHOLD);
    }

    void refreshChatSigh();
    const interval = window.setInterval(() => void refreshChatSigh(), CHAT_SIGH_POLL_MS);
    const unlistenRoom = listenForChatRoomSelection((selection) => {
      roomId = selection.rootId;
      void refreshChatSigh();
    });
    const unlistenChatClosed = listen(CHAT_OVERLAY_CLOSED_EVENT, () => void refreshChatSigh());

    return () => {
      cancelled = true;
      window.clearInterval(interval);
      void unlistenRoom.then((off) => off());
      void unlistenChatClosed.then((off) => off());
    };
  }, []);

  useEffect(() => {
    const unlisten = listenForSpeechBubbleClosed(() => setSpeechActive(false));
    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

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
    if (dragAngryTimer.current !== null) {
      window.clearTimeout(dragAngryTimer.current);
      dragAngryTimer.current = null;
    }
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
    lastDragPoint.current = null;
    dragOrigin.current = null;
    dragMode.current = "idle";
    draggingStarted.current = false;
    setIsDraggingOverlay(false);
    setPointerHeld(false);
  }

  // The mouse only tolerates being held for so long: once `DRAG_TOO_LONG_MS` elapses mid-drag, it
  // wriggles free (as if the user let go) and gets mad for a bit.
  function triggerDragAngry() {
    void releaseDrag();
    setAngryUntil(Date.now() + ANGRY_DURATION_MS);
  }

  useEffect(() => {
    const onPointerMoveGlobal = (pointerEvent: globalThis.PointerEvent) => {
      const start = dragStart.current;
      if (!start || !draggingStarted.current) return;

      void isCurrentWindowOverHouse()
        .then((active) => setHouseDropTarget(active))
        .catch(() => undefined);
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

  const angryActive = angryUntil !== null;
  const wanderActive =
    event.kind === "IDLE" &&
    !isDraggingOverlay &&
    !pointerHeld &&
    !chatOpen &&
    !angryActive &&
    !chatSighActive &&
    !speechActive;
  const { phase: wanderPhase, facing } = useCharacterWander(wanderActive, dragReleaseTick);

  const motionUrl = useMemo(() => {
    if (isDraggingOverlay) return danglingMotionUrl;
    if (angryActive) return angryMotionUrl;
    if (event.kind !== "IDLE") return overlayMotionUrls[event.kind];
    if (chatSighActive) return sighMotionUrl;
    if (wanderActive) return wanderMotionUrls[wanderPhase];
    return overlayMotionUrls.IDLE;
  }, [angryActive, chatSighActive, event.kind, isDraggingOverlay, wanderActive, wanderPhase]);

  const spriteFlip = wanderActive && facing === 1 ? "scaleX(-1)" : "none";

  // If a more urgent state interrupts an open speech bubble, close it immediately rather than
  // leaving it stranded once the mascot moves/reacts.
  useEffect(() => {
    if (!speechActive) return;
    if (isDraggingOverlay || chatOpen || angryActive || chatSighActive) {
      setSpeechActive(false);
      void hideSpeechBubble().catch(() => undefined);
    }
  }, [speechActive, isDraggingOverlay, chatOpen, angryActive, chatSighActive]);

  // Small talk fires purely on a timer, not gated on being fully idle/wandering — otherwise it
  // rarely lines up with `wanderActive` (event.kind IDLE + not angry/sighing/etc all at once) and
  // the mouse effectively never talks. Only genuinely conflicting states block it: mid-drag (the
  // window is actively moving, so a bubble position would be stale immediately), chat open
  // (already covers that area), and an already-open bubble (avoid stacking triggers).
  const speechBlockedRef = useRef(isDraggingOverlay || chatOpen || speechActive);
  useEffect(() => {
    speechBlockedRef.current = isDraggingOverlay || chatOpen || speechActive;
  }, [isDraggingOverlay, chatOpen, speechActive]);

  useEffect(() => {
    let cancelled = false;

    function scheduleNext() {
      const delay =
        IDLE_SPEECH_MIN_DELAY_MS + Math.random() * (IDLE_SPEECH_MAX_DELAY_MS - IDLE_SPEECH_MIN_DELAY_MS);
      speechTimer.current = window.setTimeout(async () => {
        if (cancelled) return;
        if (speechBlockedRef.current) {
          scheduleNext();
          return;
        }
        const line = IDLE_SPEECH_LINES[Math.floor(Math.random() * IDLE_SPEECH_LINES.length)];
        setSpeechActive(true);
        try {
          await showSpeechBubble(line);
        } catch {
          setSpeechActive(false);
        }
        scheduleNext();
      }, delay);
    }

    scheduleNext();
    return () => {
      cancelled = true;
      if (speechTimer.current !== null) {
        window.clearTimeout(speechTimer.current);
        speechTimer.current = null;
      }
    };
  }, []);

  function beginOverlayDrag(pointer: PointerEvent<HTMLButtonElement>) {
    if (pointer.button !== 0) return;
    pointerStartedWithChatOpen.current = chatOpen;
    dragStart.current = { x: pointer.screenX, y: pointer.screenY };
    lastDragPoint.current = { x: pointer.screenX, y: pointer.screenY };
    dragOrigin.current = null;
    dragMode.current = "idle";
    draggingStarted.current = false;
    suppressClick.current = false;
    setPointerHeld(true);
    if (chatOpen) {
      setChatOpen(false);
      void hideChatOverlay().catch(() => undefined);
    }
    pointer.currentTarget.setPointerCapture(pointer.pointerId);
    void getCurrentWindow()
      .outerPosition()
      .then((position) => {
        if (dragOrigin.current) return;
        dragOrigin.current = {
          pointerX: pointer.screenX,
          pointerY: pointer.screenY,
          windowX: position.x,
          windowY: position.y
        };
      })
      .catch((cause) => {
        console.error("Failed to read character overlay position", cause);
      });
  }

  function continueOverlayDrag(pointer: PointerEvent<HTMLButtonElement>) {
    const start = dragStart.current;
    if (!start) return;
    lastDragPoint.current = { x: pointer.screenX, y: pointer.screenY };
    const moved = Math.hypot(pointer.screenX - start.x, pointer.screenY - start.y);
    if (!draggingStarted.current) {
      if (moved < DRAG_THRESHOLD_PX) return;
      draggingStarted.current = true;
      suppressClick.current = true;
      setIsDraggingOverlay(true);
      dragAngryTimer.current = window.setTimeout(triggerDragAngry, DRAG_TOO_LONG_MS);
      // Moving the mascot should not drag the chat with it, so tuck the chat away.
      setChatOpen(false);
      void hideChatOverlay().catch(() => undefined);
      dragMode.current = "manual";
      void moveOverlayFromPoint(pointer.screenX, pointer.screenY);
      return;
    }
    if (dragMode.current === "manual") {
      void moveOverlayFromPoint(pointer.screenX, pointer.screenY);
    }
  }

  function finishOverlayPointer(pointer: PointerEvent<HTMLButtonElement>) {
    const start = dragStart.current;
    const wasDragging = draggingStarted.current;
    const moved = start ? Math.hypot(pointer.screenX - start.x, pointer.screenY - start.y) : 0;
    if (pointer.currentTarget.hasPointerCapture(pointer.pointerId)) {
      pointer.currentTarget.releasePointerCapture(pointer.pointerId);
    }
    void releaseDrag();
    if (start && !wasDragging && moved < DRAG_THRESHOLD_PX) {
      if (pointerStartedWithChatOpen.current) {
        setChatOpen(false);
      } else {
        openChatOverlayFromMouse();
      }
      return;
    }
    suppressClick.current = wasDragging || moved >= DRAG_THRESHOLD_PX;
  }

  function cancelOverlayPointer(pointer: PointerEvent<HTMLButtonElement>) {
    if (pointer.currentTarget.hasPointerCapture(pointer.pointerId)) {
      pointer.currentTarget.releasePointerCapture(pointer.pointerId);
    }
    suppressClick.current = true;
    void releaseDrag();
  }

  async function moveOverlayFromPoint(screenX: number, screenY: number) {
    let origin = dragOrigin.current;
    if (!origin) {
      try {
        const position = await getCurrentWindow().outerPosition();
        origin = {
          pointerX: screenX,
          pointerY: screenY,
          windowX: position.x,
          windowY: position.y
        };
        dragOrigin.current = origin;
      } catch (cause) {
        console.error("Failed to read character overlay position", cause);
        return;
      }
    }

    const nextX = origin.windowX + (screenX - origin.pointerX);
    const nextY = origin.windowY + (screenY - origin.pointerY);
    try {
      await getCurrentWindow().setPosition(new PhysicalPosition(Math.round(nextX), Math.round(nextY)));
      await setHouseDropTarget(await isCurrentWindowOverHouse());
    } catch (cause) {
      console.error("Failed to move character overlay", cause);
    }
  }

  function openChatOverlayFromMouse() {
    if (suppressClick.current) {
      suppressClick.current = false;
      return;
    }
    void showChatOverlay()
      .then(() => {
        setChatOpen(true);
        setHasChatAttention(false);
        if (latestAutoProposal.current) {
          void publishChatAutoProposal(latestAutoProposal.current);
        }
      })
      .catch((cause) => {
        console.error("Failed to show chat overlay", cause);
        setChatOpen(false);
      });
  }

  return (
    <div className={`character-overlay character-${event.kind.toLowerCase()}`}>
      <button
        type="button"
        className="character-drag-surface"
        onPointerDown={beginOverlayDrag}
        onPointerMove={continueOverlayDrag}
        onPointerUp={finishOverlayPointer}
        onPointerCancel={cancelOverlayPointer}
        aria-label={`MouseKeeper ${KIND_LABELS[event.kind]}`}
        title="Click: chat / drag: move"
      >
        <img
          className="character-motion"
          src={motionUrl}
          alt=""
          aria-hidden="true"
          draggable={false}
          style={{ transform: spriteFlip }}
        />
        {hasChatAttention && !chatOpen ? (
          <span className="character-attention-badge" aria-hidden="true">
            !
          </span>
        ) : null}
      </button>
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
