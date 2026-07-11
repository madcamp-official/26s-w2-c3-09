import { invoke } from "@tauri-apps/api/core";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export type AgentConnectionState = "unconfigured" | "offline" | "connecting" | "online";

export type AgentErrorCode = "UNCONFIGURED" | "TRANSPORT_UNAVAILABLE";

export type AgentConnectionStatus = {
  state: AgentConnectionState;
  server_base_url: string | null;
  device_id: string | null;
  last_error_code: AgentErrorCode | null;
  last_error_message: string | null;
};

export type AgentCommand = {
  command_id: string;
  command_type: string;
  room_id: string;
  idempotency_key: string;
  payload: unknown;
};

export function getAgentConnectionStatus() {
  return invokeAgentCommand<AgentConnectionStatus>("get_agent_connection_status");
}

export function pollAgentCommands() {
  return invokeAgentCommand<AgentCommand[]>("poll_agent_commands");
}

export function sendAgentEvent(event: unknown) {
  return invokeAgentCommand<void>("send_agent_event", { event });
}

function invokeAgentCommand<T>(command: string, args?: Record<string, unknown>) {
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
