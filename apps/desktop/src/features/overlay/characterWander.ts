import { useEffect, useRef, useState } from "react";
import {
  currentMonitor,
  getAllWindows,
  getCurrentWindow,
  PhysicalPosition
} from "@tauri-apps/api/window";

import { HOUSE_OVERLAY_WINDOW_LABEL } from "./overlayApi";

/**
 * Autonomous "desktop pet" wandering for the character overlay window.
 *
 * When the mouse is free (idle, not being held or dragged) it strolls the desktop: it picks a
 * random target, walks toward it by nudging the native window position each tick, then pauses and
 * repeats — walking in random directions. The house overlay is treated as its home. Once the user
 * drops the mouse onto its floor, autonomous movement is confined to that floor until the user
 * deliberately drags it back out.
 *
 * All Tauri window calls are best-effort and swallowed on failure so a missing runtime (e.g. the
 * browser dev preview, where the overlay never renders anyway) simply produces no movement.
 */
export type WanderPhase = "walking" | "pausing" | "resting";

const TICK_MS = 55;
const STEP_PX = 3; // physical pixels moved per tick while walking
const ARRIVE_PX = 8;
const PAUSE_MS: readonly [number, number] = [700, 2200];
const REST_MS: readonly [number, number] = [4000, 9000];
// Generous cap so a walk bout can cross most of a monitor before giving up on a distant target;
// normal walks end sooner than this by simply arriving (see ARRIVE_PX).
const WALK_TIMEOUT_MS = 18000;
const HOUSE_REFRESH_MS = 2000;
// How long the mouse stays put after the user deliberately drags it into the house and lets go.
// Later walks are still allowed, but every step remains inside the house floor polygon.
const USER_PLACED_HOME_REST_MS = 1600;

type Point = { x: number; y: number };
type Size = { w: number; h: number };
type Rect = { x: number; y: number; w: number; h: number };
type Polygon = readonly Point[];

type WanderState = {
  phase: WanderPhase;
  facing: 1 | -1;
  phaseUntil: number;
  walkUntil: number;
  target: Point | null;
  pos: Point | null;
  size: Size;
  bounds: Rect | null;
  house: Rect | null;
  houseCheckedAt: number;
  needResync: boolean;
  // Set when the most recent hand-off back to the wander loop followed an actual user drag (as
  // opposed to just opening the chat bubble); consumed the next time the loop resyncs position.
  justDragEnded: boolean;
  confinedToHouse: boolean;
};

function randRange([min, max]: readonly [number, number]) {
  return min + Math.random() * (max - min);
}

function centerInside(pos: Point, size: Size, rect: Rect) {
  const cx = pos.x + size.w / 2;
  const cy = pos.y + size.h / 2;
  return cx >= rect.x && cx <= rect.x + rect.w && cy >= rect.y && cy <= rect.y + rect.h;
}

function footPoint(pos: Point, size: Size) {
  return {
    x: pos.x + size.w * 0.5,
    y: pos.y + size.h * 0.82
  };
}

function topLeftFromFoot(point: Point, size: Size) {
  return {
    x: point.x - size.w * 0.5,
    y: point.y - size.h * 0.82
  };
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

function polygonBounds(polygon: Polygon): Rect {
  const xs = polygon.map((point) => point.x);
  const ys = polygon.map((point) => point.y);
  const minX = Math.min(...xs);
  const minY = Math.min(...ys);
  const maxX = Math.max(...xs);
  const maxY = Math.max(...ys);
  return { x: minX, y: minY, w: maxX - minX, h: maxY - minY };
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

function randomPointInPolygon(polygon: Polygon) {
  const bounds = polygonBounds(polygon);
  for (let attempt = 0; attempt < 80; attempt += 1) {
    const point = {
      x: bounds.x + Math.random() * bounds.w,
      y: bounds.y + Math.random() * bounds.h
    };
    if (pointInPolygon(point, polygon)) return point;
  }
  return {
    x: bounds.x + bounds.w / 2,
    y: bounds.y + bounds.h / 2
  };
}

function closestPointInsidePolygon(point: Point, polygon: Polygon) {
  if (pointInPolygon(point, polygon)) return point;

  let closest = polygon[0];
  let closestDistance = Number.POSITIVE_INFINITY;
  for (let index = 0; index < polygon.length; index += 1) {
    const start = polygon[index];
    const end = polygon[(index + 1) % polygon.length];
    const dx = end.x - start.x;
    const dy = end.y - start.y;
    const lengthSquared = dx * dx + dy * dy;
    const ratio =
      lengthSquared === 0
        ? 0
        : Math.max(0, Math.min(1, ((point.x - start.x) * dx + (point.y - start.y) * dy) / lengthSquared));
    const candidate = { x: start.x + dx * ratio, y: start.y + dy * ratio };
    const distance = Math.hypot(candidate.x - point.x, candidate.y - point.y);
    if (distance < closestDistance) {
      closest = candidate;
      closestDistance = distance;
    }
  }

  const center = polygon.reduce(
    (sum, vertex) => ({ x: sum.x + vertex.x / polygon.length, y: sum.y + vertex.y / polygon.length }),
    { x: 0, y: 0 }
  );
  for (const inset of [2, 4, 8, 16, 24]) {
    const distance = Math.hypot(center.x - closest.x, center.y - closest.y) || 1;
    const candidate = {
      x: closest.x + ((center.x - closest.x) / distance) * inset,
      y: closest.y + ((center.y - closest.y) / distance) * inset
    };
    if (pointInPolygon(candidate, polygon)) return candidate;
  }
  return randomPointInPolygon(polygon);
}

/**
 * @param active Whether the mouse should be roaming right now (idle, not held/dragged/chatting).
 * @param dragReleaseSignal Bumped by the caller each time an actual drag (not just a click) ends.
 *   Used to tell "the user just dropped it here" apart from other reasons wandering paused.
 */
export function useCharacterWander(
  active: boolean,
  dragReleaseSignal: number
): { phase: WanderPhase; facing: 1 | -1 } {
  const [phase, setPhase] = useState<WanderPhase>("pausing");
  const [facing, setFacing] = useState<1 | -1>(1);
  const activeRef = useRef(active);
  const stateRef = useRef<WanderState>({
    phase: "pausing",
    facing: 1,
    phaseUntil: 0,
    walkUntil: 0,
    target: null,
    pos: null,
    size: { w: 184, h: 160 },
    bounds: null,
    house: null,
    houseCheckedAt: 0,
    needResync: true,
    justDragEnded: false,
    confinedToHouse: false
  });
  const dragReleaseSeenRef = useRef(dragReleaseSignal);

  useEffect(() => {
    activeRef.current = active;
  }, [active]);

  useEffect(() => {
    if (dragReleaseSignal !== dragReleaseSeenRef.current) {
      dragReleaseSeenRef.current = dragReleaseSignal;
      stateRef.current.justDragEnded = true;
    }
  }, [dragReleaseSignal]);

  useEffect(() => {
    if (!window.__TAURI_INTERNALS__) return;
    let stopped = false;
    const win = getCurrentWindow();
    const s = stateRef.current;

    const applyPhase = (next: WanderPhase) => {
      if (s.phase !== next) {
        s.phase = next;
        setPhase(next);
      }
    };
    const applyFacing = (next: 1 | -1) => {
      if (s.facing !== next) {
        s.facing = next;
        setFacing(next);
      }
    };

    async function readGeometry() {
      try {
        const size = await win.outerSize();
        s.size = { w: size.width, h: size.height };
      } catch {
        /* keep last known size */
      }
      try {
        const monitor = await currentMonitor();
        if (monitor) {
          s.bounds = {
            x: monitor.position.x,
            y: monitor.position.y,
            w: monitor.size.width,
            h: monitor.size.height
          };
        }
      } catch {
        /* keep last known bounds */
      }
    }

    async function refreshHouse(now: number) {
      if (now - s.houseCheckedAt < HOUSE_REFRESH_MS) return;
      s.houseCheckedAt = now;
      try {
        const house = (await getAllWindows()).find(
          (candidate) => candidate.label === HOUSE_OVERLAY_WINDOW_LABEL
        );
        if (!house) {
          s.house = null;
          return;
        }
        const [position, size] = await Promise.all([house.outerPosition(), house.outerSize()]);
        const houseSide = Math.min(size.width, size.height);
        const houseX = position.x + Math.max(0, size.width - houseSide);
        const houseY = position.y + Math.max(0, size.height - houseSide);
        // Keep the full square here. `houseFloorPolygon` already applies the sprite-specific
        // footprint ratios; applying another inset made the wander and drop polygons disagree.
        s.house = { x: houseX, y: houseY, w: houseSide, h: houseSide };
      } catch {
        s.house = null;
      }
    }

    function randomTarget(): Point {
      if (!s.bounds) return s.pos ?? { x: 0, y: 0 };
      const maxX = Math.max(s.bounds.x, s.bounds.x + s.bounds.w - s.size.w);
      const maxY = Math.max(s.bounds.y, s.bounds.y + s.bounds.h - s.size.h);
      return {
        x: s.bounds.x + Math.random() * (maxX - s.bounds.x),
        y: s.bounds.y + Math.random() * (maxY - s.bounds.y)
      };
    }

    function randomTargetOutsideHouse(): Point {
      for (let attempt = 0; attempt < 8; attempt += 1) {
        const candidate = randomTarget();
        if (!s.house || !centerInside(candidate, s.size, s.house)) return candidate;
      }
      return randomTarget();
    }

    function randomTargetInHouse(): Point {
      if (!s.house) return randomTarget();
      return topLeftFromFoot(randomPointInPolygon(houseFloorPolygon(s.house)), s.size);
    }

    function isStandingOnHouseFloor(pos: Point) {
      return s.house ? pointInPolygon(footPoint(pos, s.size), houseFloorPolygon(s.house)) : false;
    }

    function beginWalk(now: number, target: Point) {
      s.target = target;
      s.walkUntil = now + WALK_TIMEOUT_MS;
      applyPhase("walking");
    }

    function beginPauseOrRest(now: number, insideHouse: boolean) {
      s.target = null;
      if (insideHouse) {
        applyPhase("resting");
        s.phaseUntil = now + randRange(REST_MS);
      } else {
        applyPhase("pausing");
        s.phaseUntil = now + randRange(PAUSE_MS);
      }
    }

    async function step() {
      const now = Date.now();

      // Held, dragged, or busy with real work: hand animation control back to the caller and
      // remember to re-read the (possibly dragged) position before roaming again.
      if (!activeRef.current) {
        s.needResync = true;
        return;
      }

      if (!s.bounds || s.needResync) await readGeometry();
      await refreshHouse(now);

      if (s.needResync || !s.pos) {
        try {
          const position = await win.outerPosition();
          s.pos = { x: position.x, y: position.y };
        } catch {
          return;
        }
        s.needResync = false;
        const justDragEnded = s.justDragEnded;
        s.justDragEnded = false;
        const droppedAtHome = !!s.house && isStandingOnHouseFloor(s.pos);
        if (justDragEnded) {
          s.confinedToHouse = droppedAtHome;
        }
        if (s.confinedToHouse && s.house && !isStandingOnHouseFloor(s.pos)) {
          const targetFoot = closestPointInsidePolygon(
            footPoint(s.pos, s.size),
            houseFloorPolygon(s.house)
          );
          s.pos = topLeftFromFoot(targetFoot, s.size);
          await win
            .setPosition(new PhysicalPosition(Math.round(s.pos.x), Math.round(s.pos.y)))
            .catch(() => undefined);
        }
        if (justDragEnded && droppedAtHome) {
          // The user just dragged the mouse into the house and let go. The confinement flag stays
          // set after this rest, so later autonomous targets cannot leave the floor.
          s.target = null;
          applyPhase("resting");
          s.phaseUntil = now + USER_PLACED_HOME_REST_MS;
        } else {
          beginPauseOrRest(now, droppedAtHome);
        }
        return;
      }

      const insideHouse = !!s.house && isStandingOnHouseFloor(s.pos);

      if (s.phase === "resting") {
        if (now >= s.phaseUntil) {
          if (s.confinedToHouse && !s.house) {
            s.phaseUntil = now + randRange(REST_MS);
          } else {
            beginWalk(now, s.confinedToHouse && s.house ? randomTargetInHouse() : randomTargetOutsideHouse());
          }
        }
        return;
      }

      if (s.phase === "pausing") {
        if (now >= s.phaseUntil) {
          if (s.confinedToHouse && !s.house) {
            applyPhase("resting");
            s.phaseUntil = now + randRange(REST_MS);
          } else if (insideHouse) {
            applyPhase("resting");
            s.phaseUntil = now + randRange(REST_MS);
          } else {
            beginWalk(now, s.confinedToHouse && s.house ? randomTargetInHouse() : randomTarget());
          }
        }
        return;
      }

      // phase === "walking"
      if (!s.target) {
        beginPauseOrRest(now, insideHouse);
        return;
      }

      const dx = s.target.x - s.pos.x;
      const dy = s.target.y - s.pos.y;
      const distance = Math.hypot(dx, dy);
      if (distance <= ARRIVE_PX || now >= s.walkUntil) {
        beginPauseOrRest(now, insideHouse);
        return;
      }

      let next = {
        x: s.pos.x + (dx / distance) * STEP_PX,
        y: s.pos.y + (dy / distance) * STEP_PX
      };
      if (s.confinedToHouse && s.house) {
        const polygon = houseFloorPolygon(s.house);
        const nextFoot = footPoint(next, s.size);
        if (!pointInPolygon(nextFoot, polygon)) {
          next = topLeftFromFoot(closestPointInsidePolygon(nextFoot, polygon), s.size);
          s.pos = next;
          await win
            .setPosition(new PhysicalPosition(Math.round(next.x), Math.round(next.y)))
            .catch(() => undefined);
          beginPauseOrRest(now, true);
          return;
        }
      }
      s.pos = next;
      applyFacing(dx >= 0 ? 1 : -1);
      try {
        await win.setPosition(new PhysicalPosition(Math.round(next.x), Math.round(next.y)));
      } catch {
        /* window may be mid-transition; retry on the next tick */
      }
    }

    async function loop() {
      await readGeometry();
      while (!stopped) {
        try {
          await step();
        } catch {
          /* never let one bad tick kill the wander loop */
        }
        await new Promise((resolve) => setTimeout(resolve, TICK_MS));
      }
    }

    void loop();
    return () => {
      stopped = true;
    };
  }, []);

  return { phase, facing };
}
