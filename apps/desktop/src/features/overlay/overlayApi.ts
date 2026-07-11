import { invoke } from "@tauri-apps/api/core";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export type OverlayState = "not_ready" | "hidden" | "visible" | "error";

export type OverlayErrorCode = "NOT_READY" | "WINDOW_MISSING" | "EMIT_FAILED";

export type CharacterEventKind =
  | "idle"
  | "analyzing"
  | "waiting_for_approval"
  | "working"
  | "success"
  | "error";

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

export function getOverlayStatus() {
  return invokeOverlayCommand<OverlayStatus>("get_overlay_status");
}

export function emitCharacterEvent(event: CharacterEvent) {
  return invokeOverlayCommand<void>("emit_character_event", { event });
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
