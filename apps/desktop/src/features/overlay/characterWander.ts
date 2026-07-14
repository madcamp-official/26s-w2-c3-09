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
 * repeats — walking in random directions. The house overlay is treated as its home: while the
 * mouse is standing inside the house it rests instead of roaming, and leaves again after a while.
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
// The mouse must stand well inside the house footprint to count as "home", so it does not rest the
// instant it clips the outer edge of the (large, mostly-transparent) house sprite.
const HOUSE_INSET = 0.2;
// How long the mouse stays put after the user deliberately drags it into the house and lets go —
// distinct from the brief REST_MS pause it takes when it wanders home on its own.
const USER_PLACED_HOME_REST_MS = 5 * 60 * 1000;

type Point = { x: number; y: number };
type Size = { w: number; h: number };
type Rect = { x: number; y: number; w: number; h: number };

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
};

function randRange([min, max]: readonly [number, number]) {
  return min + Math.random() * (max - min);
}

function centerInside(pos: Point, size: Size, rect: Rect) {
  const cx = pos.x + size.w / 2;
  const cy = pos.y + size.h / 2;
  return cx >= rect.x && cx <= rect.x + rect.w && cy >= rect.y && cy <= rect.y + rect.h;
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
    justDragEnded: false
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
        s.house = {
          x: position.x + size.width * HOUSE_INSET,
          y: position.y + size.height * HOUSE_INSET,
          w: size.width * (1 - HOUSE_INSET * 2),
          h: size.height * (1 - HOUSE_INSET * 2)
        };
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

      if (!s.bounds) await readGeometry();
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
        const droppedAtHome = s.house ? centerInside(s.pos, s.size, s.house) : false;
        if (justDragEnded && droppedAtHome) {
          // The user just dragged the mouse into the house and let go — keep it home for a long
          // cooldown instead of the brief natural rest.
          s.target = null;
          applyPhase("resting");
          s.phaseUntil = now + USER_PLACED_HOME_REST_MS;
        } else {
          beginPauseOrRest(now, droppedAtHome);
        }
        return;
      }

      const insideHouse = s.house ? centerInside(s.pos, s.size, s.house) : false;

      if (s.phase === "resting") {
        if (now >= s.phaseUntil) beginWalk(now, randomTargetOutsideHouse());
        return;
      }

      if (s.phase === "pausing") {
        if (now >= s.phaseUntil) {
          if (insideHouse) {
            applyPhase("resting");
            s.phaseUntil = now + randRange(REST_MS);
          } else {
            beginWalk(now, randomTarget());
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

      const nextX = s.pos.x + (dx / distance) * STEP_PX;
      const nextY = s.pos.y + (dy / distance) * STEP_PX;
      s.pos = { x: nextX, y: nextY };
      applyFacing(dx >= 0 ? 1 : -1);
      try {
        await win.setPosition(new PhysicalPosition(Math.round(nextX), Math.round(nextY)));
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
