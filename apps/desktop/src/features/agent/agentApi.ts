import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AutoCleanupProposalEvent } from "../files/fileEngineApi";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export type AgentConnectionState = "unconfigured" | "offline" | "connecting" | "online" | "revoked";

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

export type AgentChatSession = {
  session_id: string;
  room_id: string;
  title: string;
  status: "ACTIVE";
  created_at: string;
  updated_at: string;
  message_preview: string;
  unread_count: number;
  pending_action_count: number;
  last_read_message_id: string | null;
  read_at: string | null;
};

export type AgentChatMessage = {
  message_id: string;
  room_id: string;
  session_id: string | null;
  sender_type: "USER" | "ASSISTANT";
  message_type: "TEXT" | "COMMAND_DRAFT" | "RULE_DRAFT" | "PROPOSAL" | "QUERY_RESULT" | "EXECUTION_RESULT";
  content: string;
  structured_payload: unknown;
  command_id: string | null;
  created_at: string;
};

export type AgentChatSendResult = {
  message: AgentChatMessage;
  assistant: AgentChatMessage | null;
  ai_status: string;
  ai: unknown;
};

export type AgentChatQuickPrompt = {
  id: string;
  label: string;
  prompt: string;
  category: "QUERY" | "COMMAND" | "RULE" | "CLEANUP";
};

export type AgentChatQuickHistoryItem = {
  message_id: string;
  session_id: string;
  session_title: string;
  sender_type: "USER" | "ASSISTANT" | "SYSTEM";
  message_type: AgentChatMessage["message_type"];
  content: string;
  created_at: string;
};

export type AgentChatQuickSuggestion = {
  message_id: string;
  session_id: string;
  session_title: string;
  message_type: "COMMAND_DRAFT" | "RULE_DRAFT" | "PROPOSAL";
  content: string;
  draft_id: string;
  status: string;
  created_at: string;
};

export type AgentChatQuickView = {
  prompts: AgentChatQuickPrompt[];
  sessions: AgentChatSession[];
  history: AgentChatQuickHistoryItem[];
  pending_suggestions: AgentChatQuickSuggestion[];
  unread_count: number;
  pending_action_count: number;
};

export type AgentChatQuickCleanupResult = AgentChatSendResult & {
  session: AgentChatSession;
};

export type AgentRuleDraftSummary = {
  draft_id: string;
  status: string;
  rule_id: string | null;
};

export type AgentRule = {
  rule_id: string;
  room_id: string;
  name: string;
  definition: unknown;
  priority: number;
  enabled: boolean;
  version: number;
};

export type AgentRuleDraftConfirmation = {
  draft: AgentRuleDraftSummary;
  rule: AgentRule;
};

export type AgentRuleDraftRejection = {
  draft: AgentRuleDraftSummary;
};

export type CleanlinessSnapshot = {
  formulaVersion: string;
  score: number;
  metrics: {
    totalFileCount: number;
    managedFileCount: number;
    unorganizedFileCount: number;
    deductions: Array<{
      reasonCode: string;
      count: number;
      points: number;
    }>;
  };
  calculatedAt: string;
};

export type AgentRoomSnapshot = {
  snapshot_id: string;
  room_id: string;
  formula_version: string;
  score: number;
  metrics: unknown;
  calculated_at: string;
};

export type CleanlinessSnapshotSyncReport = {
  root_id: string;
  room_id: string;
  room_created: boolean;
  snapshot: CleanlinessSnapshot;
  server_snapshot: AgentRoomSnapshot;
};

export type RoomDisconnectPreflight = {
  root_id: string;
  room_id: string;
  blocking_reasons: string[];
  undoable_operation_count: number;
  requires_confirmation: boolean;
};

export type RoomDisconnectReport = {
  root_id: string;
  room_id: string;
  watcher_stopped: boolean;
  index_cleared: boolean;
  undoable_operation_count: number;
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
  last_file_browse_poll_unix_ms: number | null;
  last_file_browse_count: number;
  last_file_browse_completed_count: number;
  last_file_browse_failed_count: number;
  last_file_index_reconcile_unix_ms: number | null;
  last_file_index_reconcile_root_count: number;
  last_file_index_reconcile_failed_count: number;
  last_file_transfer_poll_unix_ms: number | null;
  last_file_transfer_count: number;
  last_file_transfer_uploaded_count: number;
  last_file_transfer_failed_count: number;
  last_smart_cache_poll_unix_ms: number | null;
  last_smart_cache_candidate_count: number;
  last_smart_cache_uploaded_count: number;
  last_smart_cache_failed_count: number;
  last_auto_cleanup_unix_ms: number | null;
  last_auto_cleanup_root_count: number;
  last_auto_cleanup_approved_count: number;
  last_auto_cleanup_executed_count: number;
  last_auto_cleanup_failed_count: number;
  last_auto_cleanup_proposals: AutoCleanupProposalEvent[];
  last_auto_submitted_proposal_count: number;
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

export type ChatCommandDraftExecutionReport = {
  draft: {
    draft_id: string;
    command: AgentCommand | null;
  };
  command_report: CommandProcessingReport;
  proposal: {
    proposal_id: string;
    command_id: string;
    room_id: string;
    status: string;
    item_ids: string[];
  };
  decision: {
    decision_id: string;
    proposal_id: string;
    decision_type: string;
  };
  execution_report: DecisionProcessingReport;
  proposal_outbox_report: OutboxFlushReport;
  execution_outbox_report: OutboxFlushReport;
};

export type AgentOpenProposal = {
  proposal_id: string;
  command_id: string;
  room_id: string;
  status: string;
};

export type ChatProposalExecutionReport = {
  proposal: {
    proposal_id: string;
    command_id: string;
    room_id: string;
    status: string;
    item_ids: string[];
  };
  decision: {
    decision_id: string;
    proposal_id: string;
    decision_type: string;
  };
  execution_report: DecisionProcessingReport;
  execution_outbox_report: OutboxFlushReport;
};

export type FileBrowseProcessingStatus = "completed" | "failed";

export type FileBrowseProcessingResult = {
  request_id: string;
  status: FileBrowseProcessingStatus;
  entry_count: number;
  next_cursor: string | null;
  message: string | null;
};

export type FileBrowseProcessingReport = {
  inspected_count: number;
  completed_count: number;
  failed_count: number;
  results: FileBrowseProcessingResult[];
};

export type FileTransferProcessingStatus = "completed" | "failed" | "skipped";

export type FileTransferProcessingResult = {
  transfer_id: string;
  status: FileTransferProcessingStatus;
  size_bytes: number | null;
  sha256: string | null;
  failure_code: string | null;
  failure_reported: boolean;
  message: string | null;
};

export type FileTransferProcessingReport = {
  inspected_count: number;
  uploaded_count: number;
  failed_count: number;
  skipped_count: number;
  results: FileTransferProcessingResult[];
};

export type SmartCacheProcessingReport = {
  inspected_count: number;
  submitted_count: number;
  approved_count: number;
  uploaded_count: number;
  failed_count: number;
  skipped_count: number;
  message: string | null;
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

export function approveAgentCommandDraftAndExecute(
  draftId: string,
  roomId: string,
  idempotencyKey: string
) {
  return invokeAgentCommand<ChatCommandDraftExecutionReport>(
    "approve_agent_command_draft_and_execute",
    { draftId, roomId, idempotencyKey }
  );
}

export function confirmAgentRuleDraft(
  draftId: string,
  roomId: string,
  idempotencyKey: string
) {
  return invokeAgentCommand<AgentRuleDraftConfirmation>(
    "confirm_agent_rule_draft",
    { draftId, roomId, idempotencyKey }
  );
}

export function rejectAgentRuleDraft(draftId: string) {
  return invokeAgentCommand<AgentRuleDraftRejection>("reject_agent_rule_draft", { draftId });
}

export function listAgentOpenProposals(roomId: string) {
  return invokeAgentCommand<AgentOpenProposal[]>("list_agent_open_proposals", { roomId });
}

export function approveAgentOpenProposalAndExecute(
  proposalId: string,
  roomId: string,
  idempotencyKey: string
) {
  return invokeAgentCommand<ChatProposalExecutionReport>(
    "approve_agent_open_proposal_and_execute",
    { proposalId, roomId, idempotencyKey }
  );
}

export function processAgentFileBrowseRequests() {
  return invokeAgentCommand<FileBrowseProcessingReport>("process_agent_file_browse_requests");
}

export function processAgentFileTransfers() {
  return invokeAgentCommand<FileTransferProcessingReport>("process_agent_file_transfers");
}

export function processSmartCacheForRoom(roomId: string, limit = 25) {
  return invokeAgentCommand<SmartCacheProcessingReport>("process_smart_cache_for_room", {
    roomId,
    limit
  });
}

export function flushAgentOutbox() {
  return invokeAgentCommand<OutboxFlushReport>("flush_agent_outbox");
}

export function ensureAgentRoom(rootId: string, displayName: string) {
  return invokeAgentCommand<AgentRoomSync>("ensure_agent_room", { rootId, displayName });
}

export function listAgentChatSessions(roomId: string) {
  return invokeAgentCommand<AgentChatSession[]>("list_agent_chat_sessions", { roomId });
}

export function createAgentChatSession(roomId: string, title?: string) {
  return invokeAgentCommand<AgentChatSession>("create_agent_chat_session", {
    roomId,
    title: title ?? null
  });
}

export function getAgentChatQuickView(roomId: string) {
  return invokeAgentCommand<AgentChatQuickView>("get_agent_chat_quick_view", { roomId });
}

export function createAgentChatQuickCleanup(roomId: string) {
  return invokeAgentCommand<AgentChatQuickCleanupResult>("create_agent_chat_quick_cleanup", { roomId });
}

export function listAgentChatMessages(sessionId: string) {
  return invokeAgentCommand<AgentChatMessage[]>("list_agent_chat_messages", { sessionId });
}

export function markAgentChatSessionRead(sessionId: string, lastReadMessageId?: string | null) {
  return invokeAgentCommand<AgentChatSession>("mark_agent_chat_session_read", {
    sessionId,
    lastReadMessageId: lastReadMessageId ?? null
  });
}

export function sendAgentChatMessage(sessionId: string, content: string) {
  return invokeAgentCommand<AgentChatSendResult>("send_agent_chat_message", {
    sessionId,
    content
  });
}

export function submitCleanlinessSnapshot(rootId: string) {
  return invokeAgentCommand<CleanlinessSnapshotSyncReport>("submit_cleanliness_snapshot", {
    rootId
  });
}

export function replayAgentEvents() {
  return invokeAgentCommand<SyncReplay>("replay_agent_events");
}

export function updateAgentCommandStatus(commandId: string, status: string) {
  return invokeAgentCommand<AgentCommand>("update_agent_command_status", { commandId, status });
}

export function revokeAgentDevice(idempotencyKey: string) {
  return invokeAgentCommand<AgentConnectionStatus>("revoke_agent_device", { idempotencyKey });
}

export function preflightAgentRoomDisconnect(rootId: string) {
  return invokeAgentCommand<RoomDisconnectPreflight>("preflight_agent_room_disconnect", {
    rootId
  });
}

export function disconnectAgentRoom(
  rootId: string,
  idempotencyKey: string,
  acknowledgeUndoable: boolean
) {
  return invokeAgentCommand<RoomDisconnectReport>("disconnect_agent_room", {
    rootId,
    idempotencyKey,
    acknowledgeUndoable
  });
}

export function listenForDesktopDeviceRevoked(handler: () => void) {
  ensureTauriRuntime();
  return listen<string | null>("desktop-device-revoked", () => handler());
}

function invokeAgentCommand<T>(command: string, args?: Record<string, unknown>) {
  ensureTauriRuntime();
  return invoke<T>(command, args);
}

function ensureTauriRuntime() {
  if (!window.__TAURI_INTERNALS__) {
    throw new Error(
      "Tauri runtime is not available. Run `pnpm --filter @mousekeeper/desktop tauri:dev` from the repository root."
    );
  }
}
