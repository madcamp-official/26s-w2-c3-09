import { useEffect, useMemo, useRef, useState } from "react";
import {
  disable as disableAutostart,
  enable as enableAutostart,
  isEnabled as isAutostartEnabled
} from "@tauri-apps/plugin-autostart";
import { QRCodeSVG } from "qrcode.react";

import {
  AgentCommand,
  AgentConnectionStatus,
  BackgroundRuntimeStatus,
  forgetAgentDevice,
  getAgentConnectionStatus,
  getBackgroundRuntimeStatus,
  PairingSession,
  pauseBackgroundRuntime,
  processAgentCommands,
  processAgentDecisions,
  processAgentFileBrowseRequests,
  processAgentFileTransfers,
  processSmartCacheForRoom,
  flushAgentOutbox,
  pollAgentCommands,
  pollAgentPairing,
  replayAgentEvents,
  startBackgroundRuntime,
  startAgentPairing,
  updateAgentCommandStatus
} from "./agentApi";
import {
  MouseKeeperMotion,
  mousekeeperMotionUrls,
  motionForAgent,
  motionFromSyncEvents
} from "../character/characterMotion";

const backgroundRefreshIntervalMs = 15_000;
// The server allows ten pairing requests per minute. One create, one mobile claim,
// and polling must all fit in that shared budget.
const pairingPollIntervalMs = 10_000;
export function AgentPanel() {
  const [connection, setConnection] = useState<AgentConnectionStatus | null>(null);
  const [background, setBackground] = useState<BackgroundRuntimeStatus | null>(null);
  const [deviceName, setDeviceName] = useState("MouseKeeper Desktop");
  const [pairing, setPairing] = useState<PairingSession | null>(null);
  const [commands, setCommands] = useState<AgentCommand[]>([]);
  const [smartCacheRoomId, setSmartCacheRoomId] = useState("");
  const [syncCursor, setSyncCursor] = useState<number | null>(null);
  const [lastReplayCount, setLastReplayCount] = useState(0);
  const [replayMotion, setReplayMotion] = useState<MouseKeeperMotion | null>(null);
  const [lastProcessedSummary, setLastProcessedSummary] = useState<string | null>(null);
  const [autostart, setAutostart] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const pairingRequestInFlight = useRef(false);
  const pairingPayload = useMemo(
    () => (pairing && connection?.server_base_url ? pairingQrPayload(pairing, connection.server_base_url) : null),
    [connection?.server_base_url, pairing]
  );

  useEffect(() => {
    void refreshConnection();
    void refreshAutostart();
    void refreshBackground();
  }, []);

  useEffect(() => {
    if (!pairing) return;

    const poll = async () => {
      if (new Date(pairing.expires_at).getTime() <= Date.now()) {
        setPairing(null);
        setError("Pairing code expired. Start a new pairing session.");
        return;
      }
      if (pairingRequestInFlight.current) return;
      pairingRequestInFlight.current = true;
      try {
        const result = await pollAgentPairing(pairing.session_id, pairing.desktop_nonce);
        setError(null);
        if (result.status === "CLAIMED") {
          setPairing(null);
          setBackground(await startBackgroundRuntime());
          await replayEvents();
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

    void resumeBackgroundRuntime({ silent: true });
    const timer = window.setInterval(() => {
      void refreshConnection();
      void refreshBackground();
    }, backgroundRefreshIntervalMs);
    return () => window.clearInterval(timer);
  }, [connection?.device_id]);

  async function refreshConnection() {
    try {
      setConnection(await getAgentConnectionStatus());
    } catch (cause) {
      setError(errorMessage(cause));
    }
  }

  async function refreshBackground() {
    try {
      setBackground(await getBackgroundRuntimeStatus());
    } catch (cause) {
      setError(errorMessage(cause));
    }
  }

  async function resumeBackgroundRuntime(options: { silent?: boolean } = {}) {
    if (!options.silent) {
      setBusy(true);
      setError(null);
    }
    try {
      setBackground(await startBackgroundRuntime());
      await refreshConnection();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      if (!options.silent) {
        setBusy(false);
      }
    }
  }

  async function pauseBackground() {
    setBusy(true);
    setError(null);
    try {
      setBackground(await pauseBackgroundRuntime());
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
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

  async function processCommandsNow() {
    setBusy(true);
    setError(null);
    try {
      const report = await processAgentCommands();
      setLastProcessedSummary(
        `${report.submitted_proposal_count} proposal batch(es), ${report.failed_count} failed, ${report.skipped_count} skipped`
      );
      setCommands(await pollAgentCommands());
      await refreshBackground();
      await refreshConnection();
    } catch (cause) {
      setError(errorMessage(cause));
      await refreshConnection();
    } finally {
      setBusy(false);
    }
  }

  async function processDecisionsNow() {
    setBusy(true);
    setError(null);
    try {
      const report = await processAgentDecisions();
      setLastProcessedSummary(
        `${report.processed_count} execution(s) completed, ${report.failed_count} failed, ${report.executed_item_count} item(s) executed`
      );
      await refreshBackground();
      await refreshConnection();
    } catch (cause) {
      setError(errorMessage(cause));
      await refreshConnection();
    } finally {
      setBusy(false);
    }
  }

  async function processFileBrowseNow() {
    setBusy(true);
    setError(null);
    try {
      const report = await processAgentFileBrowseRequests();
      setLastProcessedSummary(
        `file browse: ${report.completed_count} completed, ${report.failed_count} failed of ${report.inspected_count}`
      );
      await refreshBackground();
      await refreshConnection();
    } catch (cause) {
      setError(errorMessage(cause));
      await refreshConnection();
    } finally {
      setBusy(false);
    }
  }

  async function processFileTransfersNow() {
    setBusy(true);
    setError(null);
    try {
      const report = await processAgentFileTransfers();
      setLastProcessedSummary(
        `file transfer: ${report.uploaded_count} uploaded, ${report.failed_count} failed, ${report.skipped_count} skipped of ${report.inspected_count}`
      );
      await refreshBackground();
      await refreshConnection();
    } catch (cause) {
      setError(errorMessage(cause));
      await refreshConnection();
    } finally {
      setBusy(false);
    }
  }

  async function processSmartCacheNow() {
    setBusy(true);
    setError(null);
    try {
      const report = await processSmartCacheForRoom(smartCacheRoomId.trim());
      setLastProcessedSummary(
        `smart cache: ${report.uploaded_count} uploaded, ${report.approved_count} approved, ${report.failed_count} failed, ${report.skipped_count} skipped of ${report.inspected_count}${report.message ? ` (${report.message})` : ""}`
      );
      await refreshBackground();
      await refreshConnection();
    } catch (cause) {
      setError(errorMessage(cause));
      await refreshConnection();
    } finally {
      setBusy(false);
    }
  }

  async function flushOutboxNow() {
    setBusy(true);
    setError(null);
    try {
      const report = await flushAgentOutbox();
      setLastProcessedSummary(
        `outbox: ${report.sent_count} sent, ${report.retried_count} retried, ${report.failed_count} failed of ${report.inspected_count}`
      );
      await refreshBackground();
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
      setBackground(await pauseBackgroundRuntime());
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
            src={mousekeeperMotionUrls[mascotMotion]}
            alt={`MouseKeeper ${mascotMotion}`}
          />
          <div>
            <h2>Desktop Agent connection</h2>
            <p className="path-text">
              {connection?.server_base_url ?? "MOUSEKEEPER_SERVER_BASE_URL is not configured"}
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
          <button disabled={busy} onClick={() => void processCommandsNow()}>
            Process commands
          </button>
          <button disabled={busy} onClick={() => void processDecisionsNow()}>
            Process approved decisions
          </button>
          <button disabled={busy} onClick={() => void processFileBrowseNow()}>
            Process file browse
          </button>
          <button disabled={busy} onClick={() => void processFileTransfersNow()}>
            Process file transfers
          </button>
          <input
            aria-label="Smart cache room id"
            placeholder="Room id for smart cache"
            value={smartCacheRoomId}
            onChange={(event) => setSmartCacheRoomId(event.target.value)}
          />
          <button disabled={busy || !smartCacheRoomId.trim()} onClick={() => void processSmartCacheNow()}>
            Process smart cache
          </button>
          <button disabled={busy} onClick={() => void flushOutboxNow()}>
            Flush outbox
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

      {background ? (
        <div className="background-runtime">
          <div>
            <strong>Background runtime</strong>
            <small>
              {background.state} | commands {background.last_command_count} | heartbeat{" "}
              {formatRuntimeTime(background.last_heartbeat_unix_ms)}
            </small>
            <small>
              processed {background.last_processed_command_count} | submitted proposals{" "}
              {background.last_submitted_proposal_count}
            </small>
            <small>
              replay {formatRuntimeTime(background.last_replay_unix_ms)} | command poll{" "}
              {formatRuntimeTime(background.last_command_poll_unix_ms)}
            </small>
            <small>
              decisions {background.last_decision_count} | executed items{" "}
              {background.last_executed_item_count} | execution failures{" "}
              {background.last_execution_failed_count} | decision poll{" "}
              {formatRuntimeTime(background.last_decision_poll_unix_ms)}
            </small>
            <small>
              realtime signal {formatRuntimeTime(background.last_realtime_signal_unix_ms)}
            </small>
            <small>
              file browse {background.last_file_browse_completed_count}/
              {background.last_file_browse_count} | failed{" "}
              {background.last_file_browse_failed_count} | poll{" "}
              {formatRuntimeTime(background.last_file_browse_poll_unix_ms)}
            </small>
            <small>
              file transfers {background.last_file_transfer_uploaded_count}/
              {background.last_file_transfer_count} | failed{" "}
              {background.last_file_transfer_failed_count} | poll{" "}
              {formatRuntimeTime(background.last_file_transfer_poll_unix_ms)}
            </small>
            <small>
              smart cache {background.last_smart_cache_uploaded_count}/
              {background.last_smart_cache_candidate_count} | failed{" "}
              {background.last_smart_cache_failed_count} | poll{" "}
              {formatRuntimeTime(background.last_smart_cache_poll_unix_ms)}
            </small>
            <small>
              outbox sent {background.last_outbox_sent_count} | failed{" "}
              {background.last_outbox_failed_count} | flush{" "}
              {formatRuntimeTime(background.last_outbox_flush_unix_ms)}
            </small>
            {lastProcessedSummary ? <small>{lastProcessedSummary}</small> : null}
            {background.last_error_message ? (
              <small className="error-text">{background.last_error_message}</small>
            ) : null}
          </div>
          <div className="runtime-actions">
            <button disabled={busy || !connection?.device_id} onClick={() => void resumeBackgroundRuntime()}>
              Resume
            </button>
            <button disabled={busy} onClick={() => void pauseBackground()}>
              Pause
            </button>
          </div>
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
          Start MouseKeeper when I sign in to this computer
        </label>
      ) : null}

      {pairing ? (
        <div className="pairing-code" role="status">
          <div className="pairing-qr-panel">
            {pairingPayload ? (
              <QRCodeSVG
                className="pairing-qr"
                value={pairingPayload}
                size={156}
                level="M"
                includeMargin
                aria-label="MouseKeeper pairing QR code"
              />
            ) : null}
            <div className="pairing-code-copy">
              <span>Scan this QR or enter this code in the signed-in mobile app</span>
              <strong>{pairing.code}</strong>
              <small>Expires: {new Date(pairing.expires_at).toLocaleString()}</small>
              <small>QR contains only pairing claim data. It never contains the device token.</small>
            </div>
          </div>
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
                  {command.status} | room {command.room_id}
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

function pairingQrPayload(pairing: PairingSession, serverBaseUrl: string) {
  return JSON.stringify({
    type: "mousekeeper_pairing",
    version: 1,
    code: pairing.code,
    sessionId: pairing.session_id,
    serverBaseUrl,
    expiresAt: pairing.expires_at
  });
}

function formatRuntimeTime(value: number | null) {
  return value ? new Date(value).toLocaleTimeString() : "never";
}
