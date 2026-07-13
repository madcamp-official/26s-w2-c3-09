import { useEffect, useRef, useState } from "react";
import {
  disable as disableAutostart,
  enable as enableAutostart,
  isEnabled as isAutostartEnabled
} from "@tauri-apps/plugin-autostart";

import {
  AgentCommand,
  AgentConnectionStatus,
  forgetAgentDevice,
  getAgentConnectionStatus,
  PairingSession,
  pollAgentCommands,
  pollAgentPairing,
  replayAgentEvents,
  sendAgentHeartbeat,
  startAgentPairing,
  updateAgentCommandStatus
} from "./agentApi";
import {
  HousemouseMotion,
  housemouseMotionUrls,
  motionForAgent,
  motionFromSyncEvents
} from "../character/characterMotion";

const heartbeatIntervalMs = 15_000;
// The server allows ten pairing requests per minute. One create, one mobile claim,
// and polling must all fit in that shared budget.
const pairingPollIntervalMs = 10_000;
export function AgentPanel() {
  const [connection, setConnection] = useState<AgentConnectionStatus | null>(null);
  const [deviceName, setDeviceName] = useState("HouseMouse Desktop");
  const [pairing, setPairing] = useState<PairingSession | null>(null);
  const [commands, setCommands] = useState<AgentCommand[]>([]);
  const [syncCursor, setSyncCursor] = useState<number | null>(null);
  const [lastReplayCount, setLastReplayCount] = useState(0);
  const [replayMotion, setReplayMotion] = useState<HousemouseMotion | null>(null);
  const [autostart, setAutostart] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const pairingRequestInFlight = useRef(false);

  useEffect(() => {
    void refreshConnection();
    void refreshAutostart();
  }, []);

  useEffect(() => {
    if (!pairing) return;

    const poll = async () => {
      if (pairingRequestInFlight.current) return;
      pairingRequestInFlight.current = true;
      try {
        const result = await pollAgentPairing(pairing.session_id, pairing.desktop_nonce);
        setError(null);
        if (result.status === "CLAIMED") {
          setPairing(null);
          await refreshConnection();
        } else {
          await refreshConnection();
        }
      } catch (cause) {
        setError(errorMessage(cause));
      } finally {
        pairingRequestInFlight.current = false;
      }
    };

    void poll();
    const timer = window.setInterval(() => void poll(), pairingPollIntervalMs);
    return () => window.clearInterval(timer);
  }, [pairing]);

  useEffect(() => {
    if (!connection?.device_id) return;

    const heartbeat = async () => {
      try {
        await sendAgentHeartbeat("ONLINE_IDLE");
        await replayEvents();
        await refreshConnection();
      } catch (cause) {
        setError(errorMessage(cause));
        await refreshConnection();
      }
    };

    void heartbeat();
    const timer = window.setInterval(() => void heartbeat(), heartbeatIntervalMs);
    return () => window.clearInterval(timer);
  }, [connection?.device_id]);

  async function refreshConnection() {
    try {
      setConnection(await getAgentConnectionStatus());
    } catch (cause) {
      setError(errorMessage(cause));
    }
  }

  async function refreshAutostart() {
    try {
      setAutostart(await isAutostartEnabled());
    } catch (cause) {
      setError(errorMessage(cause));
    }
  }

  async function changeAutostart(enabled: boolean) {
    setBusy(true);
    setError(null);
    try {
      if (enabled) {
        await enableAutostart();
      } else {
        await disableAutostart();
      }
      setAutostart(await isAutostartEnabled());
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }

  async function beginPairing() {
    setBusy(true);
    setError(null);
    try {
      const session = await startAgentPairing(deviceName);
      setPairing(session);
      await refreshConnection();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }

  async function refreshCommands() {
    setBusy(true);
    setError(null);
    try {
      setCommands(await pollAgentCommands());
      await replayEvents();
      await refreshConnection();
    } catch (cause) {
      setError(errorMessage(cause));
      await refreshConnection();
    } finally {
      setBusy(false);
    }
  }

  async function replayEvents() {
    const replay = await replayAgentEvents();
    setSyncCursor(replay.next_cursor);
    setLastReplayCount(replay.events.length);
    const motion = motionFromSyncEvents(replay.events);
    if (motion) setReplayMotion(motion);
    if (replay.events.some((event) => event.event_type === "command.available")) {
      setCommands(await pollAgentCommands());
    }
  }

  async function advanceCommand(command: AgentCommand, status: string) {
    setBusy(true);
    setError(null);
    try {
      const updated = await updateAgentCommandStatus(command.command_id, status);
      setCommands((current) =>
        current.map((item) => (item.command_id === updated.command_id ? updated : item))
      );
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }

  async function forgetDevice() {
    setBusy(true);
    setError(null);
    try {
      setConnection(await forgetAgentDevice());
      setPairing(null);
      setCommands([]);
      setSyncCursor(null);
      setLastReplayCount(0);
      setReplayMotion(null);
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }

  const mascotMotion = motionForAgent({
    connection,
    pairing: pairing !== null,
    commandStatuses: commands.map((command) => command.status),
    replayMotion,
    localError: error !== null
  });

  return (
    <section className="panel agent-panel">
      <div className="section-header">
        <div className="mascot-heading">
          <img
            className={`mascot-image motion-${mascotMotion}`}
            src={housemouseMotionUrls[mascotMotion]}
            alt={`HouseMouse ${mascotMotion}`}
          />
          <div>
            <h2>Desktop Agent connection</h2>
            <p className="path-text">
              {connection?.server_base_url ?? "HOUSEMOUSE_SERVER_BASE_URL is not configured"}
            </p>
          </div>
        </div>
        <span className={`status-badge status-${connection?.state ?? "unconfigured"}`}>
          {connection?.state ?? "loading"}
        </span>
      </div>

      {connection?.device_id ? (
        <div className="agent-actions">
          <span className="path-text">Device: {connection.device_id}</span>
          {syncCursor !== null ? (
            <span className="path-text">
              Replay cursor: {syncCursor} ({lastReplayCount} new)
            </span>
          ) : null}
          <button disabled={busy} onClick={() => void refreshCommands()}>
            Refresh pending commands
          </button>
          <button className="danger-button" disabled={busy} onClick={() => void forgetDevice()}>
            Forget local pairing
          </button>
        </div>
      ) : connection?.server_base_url ? (
        <div className="input-row agent-pairing-row">
          <input
            aria-label="Desktop device name"
            maxLength={120}
            value={deviceName}
            onChange={(event) => setDeviceName(event.target.value)}
          />
          <button
            disabled={busy || pairing !== null || !deviceName.trim()}
            onClick={() => void beginPairing()}
          >
            {pairing ? "Pairing in progress" : "Start pairing"}
          </button>
        </div>
      ) : null}

      {autostart !== null ? (
        <label className="autostart-setting">
          <input
            type="checkbox"
            checked={autostart}
            disabled={busy}
            onChange={(event) => void changeAutostart(event.target.checked)}
          />
          Start Housemouse when I sign in to this computer
        </label>
      ) : null}

      {pairing ? (
        <div className="pairing-code" role="status">
          <span>Enter this code in the signed-in mobile app</span>
          <strong>{pairing.code}</strong>
          <small>Expires: {new Date(pairing.expires_at).toLocaleString()}</small>
        </div>
      ) : null}

      {connection?.last_error_message ? (
        <p className="error-text">
          {connection.last_error_code}: {connection.last_error_message}
        </p>
      ) : null}
      {error ? <p className="error-text">{error}</p> : null}

      {commands.length > 0 ? (
        <div className="command-list">
          {commands.map((command) => (
            <article className="command-row" key={command.command_id}>
              <div>
                <strong>{command.command_type}</strong>
                <small>
                  {command.status} · room {command.room_id}
                </small>
              </div>
              <div className="command-actions">
                {command.status === "QUEUED" ? (
                  <button disabled={busy} onClick={() => void advanceCommand(command, "DELIVERED")}>
                    Mark delivered
                  </button>
                ) : null}
                {command.status === "DELIVERED" ? (
                  <button disabled={busy} onClick={() => void advanceCommand(command, "ANALYZING")}>
                    Start analysis
                  </button>
                ) : null}
                {command.status === "ANALYZING" ? (
                  <button
                    className="danger-button"
                    disabled={busy}
                    onClick={() => void advanceCommand(command, "FAILED")}
                  >
                    Mark failed
                  </button>
                ) : null}
              </div>
            </article>
          ))}
        </div>
      ) : connection?.device_id ? <p className="path-text">No loaded pending commands.</p> : null}
    </section>
  );
}

function errorMessage(cause: unknown) {
  return cause instanceof Error ? cause.message : String(cause);
}
