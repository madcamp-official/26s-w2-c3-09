import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export type OverlayState = "not_ready" | "hidden" | "visible" | "error";

export type OverlayErrorCode = "NOT_READY" | "WINDOW_MISSING" | "EMIT_FAILED";

export type CharacterEventKind =
  | "IDLE"
  | "CONNECTING"
  | "ANALYZING"
  | "WAITING_APPROVAL"
  | "WORKING"
  | "SUCCESS"
  | "ERROR"
  | "USER_WORKING"
  | "OFFLINE";

export type CharacterEvent = {
  kind: CharacterEventKind;
  message?: string | null;
  correlation_id?: string | null;
};

export type OverlayStatus = {
  state: OverlayState;
  window_label: string;
  last_event_kind: CharacterEventKind | null;
  last_error_code: OverlayErrorCode | null;
  last_error_message: string | null;
};

export const OVERLAY_WINDOW_LABEL = "character-overlay";
export const CHARACTER_EVENT_NAME = "character-event";
/// Fired by the overlay chat input. This is the ONLY thing overlay chat can do: hand a bounded
/// text draft to the app for the draft/proposal flow (plan item 12). It never runs a file op.
export const OVERLAY_DRAFT_REQUEST_EVENT = "overlay:draft-request";

export type OverlayDraftRequest = {
  text: string;
};

export function getOverlayStatus() {
  return invokeOverlayCommand<OverlayStatus>("get_overlay_status");
}

export function emitCharacterEvent(event: CharacterEvent) {
  return invokeOverlayCommand<void>("emit_character_event", { event });
}

export function showOverlay() {
  return invokeOverlayCommand<OverlayStatus>("show_overlay");
}

export function hideOverlay() {
  return invokeOverlayCommand<OverlayStatus>("hide_overlay");
}

/** True when this JS context is running inside the overlay window (vs the main window). */
export function isOverlayWindow() {
  if (!window.__TAURI_INTERNALS__) return false;
  try {
    return getCurrentWindow().label === OVERLAY_WINDOW_LABEL;
  } catch {
    return false;
  }
}

/** Subscribes the overlay UI to character events pushed from the background bridge. */
export function listenForCharacterEvents(handler: (event: CharacterEvent) => void) {
  return listen<CharacterEvent>(CHARACTER_EVENT_NAME, (event) => handler(event.payload));
}

/**
 * Hands overlay chat text to the app as a draft request. Deliberately side-effect free beyond
 * emitting an event: it cannot call `trash_file`, `rename_file`, `create_file`, or execute a
 * proposal. A future AI command draft (plan item 12) consumes this event and turns it into a
 * schema-validated draft that still goes through approval / precheck / journal / execute.
 */
export async function submitOverlayDraftRequest(text: string) {
  const trimmed = text.trim();
  if (trimmed.length === 0 || trimmed.length > 2000) {
    throw new Error("Draft request must be between 1 and 2000 characters.");
  }
  await emit(OVERLAY_DRAFT_REQUEST_EVENT, { text: trimmed } satisfies OverlayDraftRequest);
}

function invokeOverlayCommand<T>(command: string, args?: Record<string, unknown>) {
  ensureTauriRuntime();

  return invoke<T>(command, args);
}

function ensureTauriRuntime() {
  if (!window.__TAURI_INTERNALS__) {
    throw new Error(
      "Tauri runtime is not available. Run the desktop app with `cargo run --features tauri-commands` from apps/desktop/src-tauri."
    );
  }
}
