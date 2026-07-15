import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { AutoCleanupProposalEvent } from "../files/fileEngineApi";

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
export const HOUSE_OVERLAY_WINDOW_LABEL = "house-overlay";
export const CHAT_OVERLAY_WINDOW_LABEL = "chat-overlay";
export const CHARACTER_EVENT_NAME = "character-event";
export const HOUSE_DROP_TARGET_EVENT = "house-drop-target";
/** Emitted whenever the chat overlay is hidden so the mascot window can keep its toggle/wander
 * state in sync, regardless of which window initiated the close. */
export const CHAT_OVERLAY_CLOSED_EVENT = "chat-overlay:closed";
export const CHAT_ROOM_SELECTED_EVENT = "chat-overlay:room-selected";
export const CHAT_AUTO_PROPOSAL_EVENT = "chat-overlay:auto-proposal";
const CHAT_ROOM_SELECTION_STORAGE_KEY = "mousekeeper.chatOverlay.selectedRootId";
/// Fired by the overlay chat input. This is the ONLY thing overlay chat can do: hand a bounded
/// text draft to the app for the draft/proposal flow (plan item 12). It never runs a file op.
export const OVERLAY_DRAFT_REQUEST_EVENT = "overlay:draft-request";

export type OverlayDraftRequest = {
  text: string;
};

export type ChatRoomSelection = {
  rootId: string | null;
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

/** Shows the standalone chat window next to the mascot. It never resizes, so the panel is never
 * clipped the way the old single-window overlay was. */
export function showChatOverlay() {
  return invokeOverlayCommand<void>("show_chat_overlay");
}

export async function hideChatOverlay() {
  try {
    await invokeOverlayCommand<void>("hide_chat_overlay");
  } finally {
    if (window.__TAURI_INTERNALS__) {
      await emit(CHAT_OVERLAY_CLOSED_EVENT).catch(() => undefined);
    }
  }
}

export async function publishChatRoomSelection(rootId: string | null) {
  const selection = { rootId } satisfies ChatRoomSelection;
  persistChatRoomSelection(selection);
  if (window.__TAURI_INTERNALS__) {
    await emit(CHAT_ROOM_SELECTED_EVENT, selection).catch(() => undefined);
  }
}

export function readChatRoomSelection(): ChatRoomSelection {
  try {
    return { rootId: window.localStorage.getItem(CHAT_ROOM_SELECTION_STORAGE_KEY) || null };
  } catch {
    return { rootId: null };
  }
}

export function listenForChatRoomSelection(handler: (selection: ChatRoomSelection) => void) {
  return listen<ChatRoomSelection>(CHAT_ROOM_SELECTED_EVENT, (event) => handler(event.payload));
}

export async function publishChatAutoProposal(proposal: AutoCleanupProposalEvent) {
  if (proposal.proposal.proposals.length === 0) return;
  if (window.__TAURI_INTERNALS__) {
    await emit(CHAT_AUTO_PROPOSAL_EVENT, proposal).catch(() => undefined);
  }
}

export function readChatAutoProposals(): AutoCleanupProposalEvent[] {
  return [];
}

export function listenForChatAutoProposals(handler: (proposal: AutoCleanupProposalEvent) => void) {
  return listen<AutoCleanupProposalEvent>(CHAT_AUTO_PROPOSAL_EVENT, (event) => {
    if (isAutoCleanupProposalEvent(event.payload)) {
      handler(event.payload);
    }
  });
}

export function setHouseOverlayLocked(locked: boolean) {
  return invokeOverlayCommand<void>("set_house_overlay_locked", { locked });
}

/** True when this JS context is running inside the overlay window (vs the main window). */
export function isOverlayWindow() {
  return isCharacterOverlayWindow() || isHouseOverlayWindow() || isChatOverlayWindow();
}

export function isChatOverlayWindow() {
  if (!window.__TAURI_INTERNALS__) return false;
  try {
    return getCurrentWindow().label === CHAT_OVERLAY_WINDOW_LABEL;
  } catch {
    return false;
  }
}

export function isCharacterOverlayWindow() {
  if (!window.__TAURI_INTERNALS__) return false;
  try {
    return getCurrentWindow().label === OVERLAY_WINDOW_LABEL;
  } catch {
    return false;
  }
}

export function isHouseOverlayWindow() {
  if (!window.__TAURI_INTERNALS__) return false;
  try {
    return getCurrentWindow().label === HOUSE_OVERLAY_WINDOW_LABEL;
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

function persistChatRoomSelection(selection: ChatRoomSelection) {
  try {
    if (selection.rootId) {
      window.localStorage.setItem(CHAT_ROOM_SELECTION_STORAGE_KEY, selection.rootId);
    } else {
      window.localStorage.removeItem(CHAT_ROOM_SELECTION_STORAGE_KEY);
    }
  } catch {
    /* localStorage may be unavailable in tests or restricted browser previews */
  }
}

function isAutoCleanupProposalEvent(value: unknown): value is AutoCleanupProposalEvent {
  if (!value || typeof value !== "object") return false;
  const record = value as Record<string, unknown>;
  if (typeof record.root_id !== "string") return false;
  const proposal = record.proposal as Record<string, unknown> | undefined;
  return (
    !!proposal &&
    typeof proposal.root === "string" &&
    Array.isArray(proposal.proposals)
  );
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
