import { invoke } from "@tauri-apps/api/core";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export type AgentConnectionState = "unconfigured" | "offline" | "connecting" | "online";

export type AgentErrorCode =
  | "UNCONFIGURED"
  | "VALIDATION_FAILED"
  | "TRANSPORT_UNAVAILABLE"
  | "UNAUTHENTICATED"
  | "FORBIDDEN"
  | "INVALID_RESPONSE"
  | "CREDENTIAL_STORE_UNAVAILABLE";

export type AgentConnectionStatus = {
  state: AgentConnectionState;
  server_base_url: string | null;
  device_id: string | null;
  last_error_code: AgentErrorCode | null;
  last_error_message: string | null;
};

export type PairingSession = {
  session_id: string;
  desktop_nonce: string;
  code: string;
  expires_at: string;
};

export type PairingStatus = {
  status: "PENDING" | "CLAIMED";
  device_id: string | null;
  expires_at: string | null;
};

export type HeartbeatResult = {
  device_id: string;
  presence: "ONLINE_IDLE" | "ONLINE_SCANNING" | "ONLINE_EXECUTING" | "DEGRADED";
  ttl_seconds: number;
};

export type AgentCommand = {
  command_id: string;
  command_type: string;
  room_id: string;
  status: string;
  payload: unknown;
};

export type AgentRoomSync = {
  room_id: string;
  root_id: string;
  name: string;
  created: boolean;
};

export type BackgroundRuntimeState = "stopped" | "running" | "suspended";

export type BackgroundRuntimeStatus = {
  state: BackgroundRuntimeState;
  last_started_unix_ms: number | null;
  last_stopped_unix_ms: number | null;
  last_heartbeat_unix_ms: number | null;
  last_replay_unix_ms: number | null;
  last_command_poll_unix_ms: number | null;
  last_command_count: number;
  last_processed_command_count: number;
  last_submitted_proposal_count: number;
  last_decision_poll_unix_ms: number | null;
  last_decision_count: number;
  last_executed_item_count: number;
  last_execution_failed_count: number;
  last_realtime_signal_unix_ms: number | null;
  last_outbox_flush_unix_ms: number | null;
  last_outbox_sent_count: number;
  last_outbox_failed_count: number;
  last_error_message: string | null;
};

export type OutboxFlushReport = {
  inspected_count: number;
  sent_count: number;
  retried_count: number;
  failed_count: number;
};

export type CommandProcessingStatus = "submitted_proposal" | "failed" | "skipped";

export type CommandProcessingResult = {
  command_id: string;
  command_type: string;
  status: CommandProcessingStatus;
  message: string | null;
  proposal_item_count: number;
};

export type CommandProcessingReport = {
  inspected_count: number;
  processed_count: number;
  submitted_proposal_count: number;
  failed_count: number;
  skipped_count: number;
  results: CommandProcessingResult[];
};

export type DecisionProcessingStatus = "completed" | "failed";

export type DecisionProcessingResult = {
  decision_id: string;
  proposal_id: string;
  status: DecisionProcessingStatus;
  message: string | null;
  executed_item_count: number;
  skipped_item_count: number;
};

export type DecisionProcessingReport = {
  inspected_count: number;
  processed_count: number;
  executed_item_count: number;
  skipped_item_count: number;
  failed_count: number;
  results: DecisionProcessingResult[];
};

export type SyncEvent = {
  event_id: string;
  event_type: string;
  schema_version: number;
  correlation_id: string;
  aggregate_type: string;
  aggregate_id: string;
  device_id: string | null;
  room_id: string | null;
  sequence: number;
  occurred_at: string;
  payload: unknown;
};

export type SyncReplay = {
  previous_cursor: number;
  next_cursor: number;
  events: SyncEvent[];
};

export function getAgentConnectionStatus() {
  return invokeAgentCommand<AgentConnectionStatus>("get_agent_connection_status");
}

export function getBackgroundRuntimeStatus() {
  return invokeAgentCommand<BackgroundRuntimeStatus>("get_background_runtime_status");
}

export function startBackgroundRuntime() {
  return invokeAgentCommand<BackgroundRuntimeStatus>("start_background_runtime");
}

export function pauseBackgroundRuntime() {
  return invokeAgentCommand<BackgroundRuntimeStatus>("pause_background_runtime");
}

export function startAgentPairing(deviceName: string) {
  return invokeAgentCommand<PairingSession>("start_agent_pairing", { deviceName });
}

export function pollAgentPairing(sessionId: string, desktopNonce: string) {
  return invokeAgentCommand<PairingStatus>("poll_agent_pairing", { sessionId, desktopNonce });
}

export function sendAgentHeartbeat(presence: HeartbeatResult["presence"] = "ONLINE_IDLE") {
  return invokeAgentCommand<HeartbeatResult>("send_agent_heartbeat", { presence });
}

export function pollAgentCommands() {
  return invokeAgentCommand<AgentCommand[]>("poll_agent_commands");
}

export function processAgentCommands() {
  return invokeAgentCommand<CommandProcessingReport>("process_agent_commands");
}

export function processAgentDecisions() {
  return invokeAgentCommand<DecisionProcessingReport>("process_agent_decisions");
}

export function flushAgentOutbox() {
  return invokeAgentCommand<OutboxFlushReport>("flush_agent_outbox");
}

export function ensureAgentRoom(rootId: string, displayName: string) {
  return invokeAgentCommand<AgentRoomSync>("ensure_agent_room", { rootId, displayName });
}

export function replayAgentEvents() {
  return invokeAgentCommand<SyncReplay>("replay_agent_events");
}

export function updateAgentCommandStatus(commandId: string, status: string) {
  return invokeAgentCommand<AgentCommand>("update_agent_command_status", { commandId, status });
}

export function forgetAgentDevice() {
  return invokeAgentCommand<AgentConnectionStatus>("forget_agent_device");
}

function invokeAgentCommand<T>(command: string, args?: Record<string, unknown>) {
  ensureTauriRuntime();
  return invoke<T>(command, args);
}

function ensureTauriRuntime() {
  if (!window.__TAURI_INTERNALS__) {
    throw new Error(
      "Tauri runtime is not available. Run `pnpm --filter @housemouse/desktop tauri:dev` from the repository root."
    );
  }
}
