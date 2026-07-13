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
const pairingPollIntervalMs = 10_000;

const connectionStateLabels: Record<string, string> = {
  unconfigured: "페어링 필요",
  offline: "오프라인",
  connecting: "연결 중",
  online: "연결됨",
  revoked: "연결 해제됨"
};

const backgroundStateLabels: Record<string, string> = {
  stopped: "중지됨",
  running: "실행 중",
  suspended: "일시정지"
};

const commandStatusLabels: Record<string, string> = {
  QUEUED: "대기 중",
  DELIVERED: "전달됨",
  ANALYZING: "분석 중",
  WAITING_APPROVAL: "승인 대기",
  APPROVED: "승인됨",
  REJECTED: "거절됨",
  EXECUTING: "실행 중",
  SUCCEEDED: "완료",
  PARTIALLY_SUCCEEDED: "일부 완료",
  FAILED: "실패",
  EXPIRED: "만료됨",
  STALE: "변경되어 중단"
};

function connectionStateLabel(state?: string | null) {
  return state ? connectionStateLabels[state] ?? state : "확인 중";
}

function commandStatusLabel(status: string) {
  return commandStatusLabels[status] ?? status;
}

function hasRecentHeartbeat(background: BackgroundRuntimeStatus | null) {
  return (
    background?.state === "running" &&
    background.last_heartbeat_unix_ms !== null &&
    Date.now() - background.last_heartbeat_unix_ms < backgroundRefreshIntervalMs * 3
  );
}

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
  const [autostartBusy, setAutostartBusy] = useState(false);
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
        setError("페어링 코드가 만료되었습니다. 다시 시작해 주세요.");
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
      if (!options.silent) setBusy(false);
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
    setAutostartBusy(true);
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
      setAutostartBusy(false);
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
        `제안 제출 ${report.submitted_proposal_count}건, 실패 ${report.failed_count}건, 건너뜀 ${report.skipped_count}건`
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
        `승인 실행 ${report.processed_count}건, 실행 항목 ${report.executed_item_count}개, 실패 ${report.failed_count}건`
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
        `파일 조회 요청 ${report.inspected_count}건 중 완료 ${report.completed_count}건, 실패 ${report.failed_count}건`
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
        `파일 전송 ${report.inspected_count}건 중 업로드 ${report.uploaded_count}건, 실패 ${report.failed_count}건, 건너뜀 ${report.skipped_count}건`
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
        `스마트 캐시 후보 ${report.inspected_count}건, 승인 ${report.approved_count}건, 업로드 ${report.uploaded_count}건, 실패 ${report.failed_count}건${report.message ? ` (${report.message})` : ""}`
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
        `보낼 작업 ${report.inspected_count}건 중 전송 ${report.sent_count}건, 재시도 ${report.retried_count}건, 실패 ${report.failed_count}건`
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

  const heartbeatOnline = hasRecentHeartbeat(background);
  const effectiveConnectionState =
    connection?.state === "offline" && heartbeatOnline ? "online" : connection?.state;
  const effectiveConnection = connection
    ? { ...connection, state: effectiveConnectionState ?? connection.state }
    : connection;
  const showConnectionError = !heartbeatOnline && connection?.last_error_message;

  const mascotMotion = motionForAgent({
    connection: effectiveConnection,
    pairing: pairing !== null,
    commandStatuses: commands.map((command) => command.status),
    replayMotion,
    localError: error !== null && !heartbeatOnline
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
            <h2>PC 연결</h2>
            <p className="path-text">
              {connection?.server_base_url ?? "서버 주소가 설정되지 않았습니다. MOUSEKEEPER_SERVER_BASE_URL을 확인하세요."}
            </p>
          </div>
        </div>
        <span className={`status-badge status-${effectiveConnectionState ?? "unconfigured"}`}>
          {connectionStateLabel(effectiveConnectionState)}
        </span>
      </div>

      {connection?.device_id ? (
        <div className="agent-actions">
          <button className="danger-button" disabled={busy} onClick={() => void forgetDevice()}>
            이 PC 연결 해제
          </button>
          <span className="path-text agent-hint">
            연결을 해제하면 모바일에서 이 PC가 사라지고, 다시 사용하려면 페어링이 필요합니다.
          </span>
        </div>
      ) : connection?.server_base_url ? (
        <div className="input-row agent-pairing-row">
          <input
            aria-label="데스크톱 기기 이름"
            placeholder="기기 이름"
            maxLength={120}
            value={deviceName}
            onChange={(event) => setDeviceName(event.target.value)}
          />
          <button
            disabled={busy || pairing !== null || !deviceName.trim()}
            onClick={() => void beginPairing()}
          >
            {pairing ? "페어링 진행 중" : "페어링 시작"}
          </button>
        </div>
      ) : null}

      {background ? (
        <>
          <div className="runtime-card">
            <div className="runtime-card-main">
              <span className={`runtime-dot runtime-${background.state}`} />
              <div>
                <strong>{backgroundStateLabels[background.state] ?? background.state}</strong>
                <small>
                  마지막 신호 {formatRuntimeTime(background.last_heartbeat_unix_ms)} · 자동 제안{" "}
                  {background.last_auto_submitted_proposal_count}건 · 자동 실행{" "}
                  {background.last_auto_cleanup_executed_count}건
                </small>
                {background.last_error_message ? (
                  <small className="error-text">{background.last_error_message}</small>
                ) : null}
              </div>
            </div>
            <div className="runtime-actions">
              <button disabled={busy} onClick={() => void resumeBackgroundRuntime()}>
                실행
              </button>
              <button disabled={busy} onClick={() => void pauseBackground()}>
                일시정지
              </button>
            </div>
          </div>

          <details className="advanced-tools">
            <summary>개발자 도구와 상세 상태</summary>
            <div className="advanced-tools-body">
              <p className="path-text">
                기기 ID: {connection?.device_id ?? "-"}
                {syncCursor !== null ? ` · 동기화 커서 ${syncCursor} · 새 이벤트 ${lastReplayCount}건` : ""}
              </p>
              <div className="agent-actions">
                <button disabled={busy} onClick={() => void refreshCommands()}>
                  명령 새로고침
                </button>
                <button disabled={busy} onClick={() => void processCommandsNow()}>
                  명령 처리
                </button>
                <button disabled={busy} onClick={() => void processDecisionsNow()}>
                  승인 처리
                </button>
                <button disabled={busy} onClick={() => void processFileBrowseNow()}>
                  파일 조회 처리
                </button>
                <button disabled={busy} onClick={() => void processFileTransfersNow()}>
                  파일 전송 처리
                </button>
                <button disabled={busy} onClick={() => void flushOutboxNow()}>
                  보낼 작업 전송
                </button>
              </div>
              <div className="input-row smart-cache-row">
                <input
                  aria-label="스마트 캐시 방 ID"
                  placeholder="스마트 캐시 방 ID"
                  value={smartCacheRoomId}
                  onChange={(event) => setSmartCacheRoomId(event.target.value)}
                />
                <button
                  disabled={busy || !smartCacheRoomId.trim()}
                  onClick={() => void processSmartCacheNow()}
                >
                  스마트 캐시 처리
                </button>
              </div>
              <div className="runtime-telemetry">
                <small>명령 {background.last_command_count}건 · 처리 {background.last_processed_command_count}건</small>
                <small>결정 {background.last_decision_count}건 · 실행 항목 {background.last_executed_item_count}개</small>
                <small>파일 조회 {background.last_file_browse_completed_count}/{background.last_file_browse_count}</small>
                <small>파일 전송 {background.last_file_transfer_uploaded_count}/{background.last_file_transfer_count}</small>
                <small>스마트 캐시 {background.last_smart_cache_uploaded_count}/{background.last_smart_cache_candidate_count}</small>
                <small>보낼 작업 {background.last_outbox_sent_count}건 · 실패 {background.last_outbox_failed_count}건</small>
                {lastProcessedSummary ? <small>{lastProcessedSummary}</small> : null}
              </div>
            </div>
          </details>
        </>
      ) : null}

      {autostart !== null ? (
        <label className="autostart-setting">
          <input
            type="checkbox"
            checked={autostart}
            disabled={autostartBusy}
            onChange={(event) => void changeAutostart(event.target.checked)}
          />
          컴퓨터에 로그인하면 MouseKeeper 자동 실행
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
                aria-label="MouseKeeper 페어링 QR 코드"
              />
            ) : null}
            <div className="pairing-code-copy">
              <span>모바일 앱에서 QR을 스캔하거나 아래 코드를 입력하세요.</span>
              <strong>{pairing.code}</strong>
              <small>만료: {new Date(pairing.expires_at).toLocaleString()}</small>
              <small>QR에는 페어링 정보만 포함되며 기기 토큰은 포함되지 않습니다.</small>
            </div>
          </div>
        </div>
      ) : null}

      {showConnectionError ? (
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
                  {commandStatusLabel(command.status)} · 방 {command.room_id}
                </small>
              </div>
              <div className="command-actions">
                {command.status === "QUEUED" ? (
                  <button disabled={busy} onClick={() => void advanceCommand(command, "DELIVERED")}>
                    전달 처리
                  </button>
                ) : null}
                {command.status === "DELIVERED" ? (
                  <button disabled={busy} onClick={() => void advanceCommand(command, "ANALYZING")}>
                    분석 시작
                  </button>
                ) : null}
                {command.status === "ANALYZING" ? (
                  <button
                    className="danger-button"
                    disabled={busy}
                    onClick={() => void advanceCommand(command, "FAILED")}
                  >
                    실패 처리
                  </button>
                ) : null}
              </div>
            </article>
          ))}
        </div>
      ) : connection?.device_id ? (
        <p className="path-text">대기 중인 명령이 없습니다.</p>
      ) : null}
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
  return value ? new Date(value).toLocaleTimeString() : "없음";
}
