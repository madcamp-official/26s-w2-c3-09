import type { AgentConnectionStatus, SyncEvent } from "../agent/agentApi";

export type MouseKeeperMotion =
  | "clean"
  | "considering"
  | "fighting"
  | "hello"
  | "sleeping"
  | "stand"
  | "walk"
  | "working";

export const mousekeeperMotionUrls: Record<MouseKeeperMotion, string> = {
  clean: new URL("../../../../../packages/character-assets/mouse_clean.png", import.meta.url).href,
  considering: new URL(
    "../../../../../packages/character-assets/mouse_considering.png",
    import.meta.url
  ).href,
  fighting: new URL(
    "../../../../../packages/character-assets/mouse_fighting.png",
    import.meta.url
  ).href,
  hello: new URL("../../../../../packages/character-assets/mouse_hello.png", import.meta.url).href,
  sleeping: new URL(
    "../../../../../packages/character-assets/mouse_sleeping.png",
    import.meta.url
  ).href,
  stand: new URL("../../../../../packages/character-assets/mouse_stand.png", import.meta.url).href,
  walk: new URL("../../../../../packages/character-assets/mouse_walk.png", import.meta.url).href,
  working: new URL(
    "../../../../../packages/character-assets/mouse_working.png",
    import.meta.url
  ).href
};

export function motionForAgent(input: {
  connection: AgentConnectionStatus | null;
  pairing: boolean;
  commandStatuses: string[];
  replayMotion: MouseKeeperMotion | null;
  localError: boolean;
}): MouseKeeperMotion {
  const statuses = new Set(input.commandStatuses);
  if (statuses.has("FAILED")) return "fighting";
  if (statuses.has("ANALYZING") || statuses.has("WAITING_APPROVAL")) return "considering";
  if (statuses.has("DELIVERED") || statuses.has("QUEUED")) return "walk";
  if (input.pairing) return "hello";
  if (input.localError && input.connection?.state !== "unconfigured") return "fighting";
  if (input.replayMotion) return input.replayMotion;
  if (input.connection?.state === "online") return "stand";
  if (input.connection?.state === "connecting") return "walk";
  return "sleeping";
}

export function motionFromSyncEvents(events: SyncEvent[]): MouseKeeperMotion | null {
  for (const event of [...events].reverse()) {
    const payload = asRecord(event.payload);
    if (event.event_type === "proposal.created") return "considering";
    if (event.event_type === "command.available") return "walk";
    if (event.event_type === "command.updated") {
      if (payload.status === "ANALYZING") return "considering";
      if (payload.status === "FAILED") return "fighting";
    }
    if (event.event_type === "execution.updated") {
      if (payload.status === "EXECUTING") return "working";
      if (payload.status === "SUCCEEDED") return "clean";
      if (
        ["PARTIALLY_SUCCEEDED", "FAILED", "STALE", "ROLLED_BACK"].includes(
          String(payload.status)
        )
      ) {
        return "fighting";
      }
    }
    if (event.event_type === "presence.updated") {
      if (payload.presence === "OFFLINE") return "sleeping";
      if (payload.presence === "ONLINE_SCANNING") return "considering";
      if (payload.presence === "ONLINE_EXECUTING") return "working";
      if (payload.presence === "DEGRADED") return "fighting";
      if (payload.presence === "ONLINE_IDLE") return "stand";
    }
  }
  return null;
}

function asRecord(value: unknown): Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}
