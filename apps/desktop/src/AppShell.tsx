import { useCallback, useEffect, useState } from "react";

import {
  getAgentConnectionStatus,
  listenForDesktopDeviceRevoked
} from "./features/agent/agentApi";
import { AgentPanel, AutostartSetting } from "./features/agent/AgentPanel";
import { listManagedRoots } from "./features/files/fileEngineApi";
import { FileEnginePanel } from "./features/files/FileEnginePanel";
import { OnboardingSlot } from "./features/onboarding/OnboardingSlot";

const splashUrl = new URL("./assets/mousekeeper-splash.png", import.meta.url).href;
const setupMascotUrl = new URL(
  "../../../packages/character-assets/new_mouse/mouse_walk_clean.png",
  import.meta.url
).href;

type DashboardSection = "rooms" | "organize" | "explore" | "history" | "connection" | "settings";
type FileSection = Exclude<DashboardSection, "connection" | "explore">;

const DASHBOARD_SECTIONS: ReadonlyArray<{
  id: DashboardSection;
  icon: string;
  label: string;
  hint: string;
}> = [
  { id: "rooms", icon: "▦", label: "방 관리", hint: "root 폴더 선택" },
  { id: "organize", icon: "▣", label: "폴더 정리", hint: "제안 · 실행" },
  { id: "explore", icon: "⌕", label: "폴더 탐색", hint: "탐색 · 검색" },
  { id: "history", icon: "▤", label: "작업 기록", hint: "결과 · 되돌리기" },
  { id: "connection", icon: "●", label: "PC 연결", hint: "페어링 · 상태" },
  { id: "settings", icon: "⚙", label: "설정", hint: "권한 · 정책" }
];

export function AppShell() {
  const [rootCount, setRootCount] = useState<number | null>(null);
  const [connectionState, setConnectionState] = useState<string | null>(null);
  const [pairingEpoch, setPairingEpoch] = useState(0);
  const [section, setSection] = useState<DashboardSection>("rooms");
  const [selectedRootId, setSelectedRootId] = useState("");

  const reloadRootCount = useCallback(async () => {
    try {
      const roots = await listManagedRoots();
      setRootCount(roots.length);
    } catch {
      setRootCount(null);
    }
  }, []);

  const refreshConnectionState = useCallback(async () => {
    try {
      const status = await getAgentConnectionStatus();
      setConnectionState(status.state);
    } catch {
      setConnectionState(null);
    }
  }, []);

  const refreshSetupState = useCallback(async () => {
    await Promise.all([reloadRootCount(), refreshConnectionState()]);
  }, [refreshConnectionState, reloadRootCount]);

  useEffect(() => {
    void refreshSetupState();
    const onFocus = () => void refreshSetupState();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [refreshSetupState]);

  useEffect(() => {
    if (!window.__TAURI_INTERNALS__) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void listenForDesktopDeviceRevoked(() => {
      // The native heartbeat is authoritative: switch the entire manager back to setup,
      // rather than creating a new code inside a dashboard panel that may be hidden.
      setConnectionState("revoked");
      setPairingEpoch((current) => current + 1);
    }).then((stop) => {
      if (cancelled) stop();
      else unlisten = stop;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const isPaired =
    connectionState === null
      ? null
      : !["unconfigured", "revoked"].includes(connectionState);

  useEffect(() => {
    if (rootCount !== null && rootCount > 0) return;
    const timer = window.setInterval(() => void refreshSetupState(), 2_000);
    return () => window.clearInterval(timer);
  }, [refreshSetupState, rootCount]);

  const focusPairing = useCallback(() => {
    document.getElementById("agent-panel")?.scrollIntoView({ behavior: "smooth" });
  }, []);

  const hasFolder = rootCount !== null && rootCount > 0;
  // A desktop-managed root is a complete local workspace even without a phone. Pairing only adds
  // remote/mobile access; it must never block the local file-safety workflow.
  const needsSetup = rootCount === null || rootCount === 0;

  return (
    <main className={`app-shell ${needsSetup ? "app-shell-setup" : ""}`}>
      <div className="app-shell-toolbar">
        <div className="app-brand-mark">
          <span className="app-brand-dot" />
          <strong className="app-title">MOUSEKEEPER Manager</strong>
        </div>
        <div className="toolbar-actions">
          <span className={`setup-mini-status ${hasFolder ? "is-ready" : ""}`}>
            {hasFolder ? `폴더 ${rootCount}개` : "폴더 필요"}
          </span>
          <span className={`setup-mini-status ${isPaired ? "is-ready" : ""}`}>
            {isPaired ? "모바일 연결됨" : "모바일 미연결 · 로컬 사용 가능"}
          </span>
        </div>
      </div>

      {needsSetup ? (
        <div className="setup-flow">
          <section className="manager-hero" aria-labelledby="manager-hero-title">
            <div className="manager-hero-art">
              <img src={splashUrl} alt="MOUSEKEEPER" />
            </div>
            <div className="manager-hero-copy">
              <span className="setup-kicker">초기 설정</span>
              <h1 id="manager-hero-title">PC 방을 MOUSEKEEPER에게 맡길 준비가 필요해요</h1>
              <p>
                정리할 root 폴더를 등록하면 PC에서 바로 매니저를 사용할 수 있습니다. 모바일
                연결은 원격 확인이 필요할 때 추가하며, 모든 파일 작업은 등록된 폴더 안에서만
                실행됩니다.
              </p>
              <div className="setup-checklist" aria-label="설치 진행 상태">
                <span className={hasFolder ? "is-done" : ""}>1. 관리 폴더 등록</span>
                <span className={isPaired ? "is-done" : ""}>2. 모바일 연결 (선택)</span>
                <span className={hasFolder ? "is-done" : ""}>3. 로컬 관리 시작</span>
              </div>
            </div>
            <img className="manager-hero-mascot" src={setupMascotUrl} alt="" aria-hidden="true" />
          </section>

          <section className="setup-grid" aria-label="초기 설정">
            <div className="setup-card setup-card-primary" id="agent-panel">
              <div className="setup-card-heading">
                <span className="setup-step-number">1</span>
                <div>
                  <h2>모바일 앱 연결</h2>
                  <p>선택 사항 · QR 또는 6자리 코드로 원격 모바일 기능을 연결합니다.</p>
                </div>
              </div>
              <AgentPanel key={`setup-pairing-${pairingEpoch}`} />
            </div>
            <div className="setup-card">
              <div className="setup-card-heading">
                <span className="setup-step-number">2</span>
                <div>
                  <h2>관리 폴더 선택</h2>
                  <p>MouseKeeper가 제안만 만들 수 있는 안전한 작업 범위를 정합니다.</p>
                </div>
              </div>
              <OnboardingSlot
                hasFolder={hasFolder}
                onRegistered={() => void refreshSetupState()}
                onFocusPairing={focusPairing}
              />
            </div>
          </section>
        </div>
      ) : (
        <div className="manager-dashboard">
          <aside className="dashboard-rail" aria-label="섹션 이동">
            <div className="dashboard-rail-brand">
              <img src={setupMascotUrl} alt="" aria-hidden="true" />
              <div>
                <strong>관리 콘솔</strong>
                <small>방을 고르고 작업을 실행합니다</small>
              </div>
            </div>
            <nav className="dashboard-nav">
              {DASHBOARD_SECTIONS.filter((item) => item.id !== "explore").map((item) => (
                <button
                  key={item.id}
                  type="button"
                  className={`dashboard-nav-item ${section === item.id ? "is-active" : ""}`}
                  aria-current={section === item.id ? "page" : undefined}
                  onClick={() => setSection(item.id)}
                >
                  <span className="dashboard-nav-icon" aria-hidden="true">
                    {item.icon}
                  </span>
                  <span className="dashboard-nav-text">
                    <strong>{item.label}</strong>
                    <small>{item.hint}</small>
                  </span>
                  {item.id === "connection" ? (
                    <span className={`dashboard-nav-dot ${isPaired ? "is-online" : ""}`} aria-hidden="true" />
                  ) : null}
                </button>
              ))}
            </nav>
          </aside>

          <div className="dashboard-main">
            <div className="dashboard-view" hidden={section !== "connection"} aria-label="PC 연결 상태">
              <AgentPanel key={`connection-pairing-${pairingEpoch}`} showAutostart={false} />
            </div>
            <div className="dashboard-view" hidden={section !== "settings"} aria-label="설정">
              <section className="panel settings-shell-panel">
                <div className="section-header">
                  <div>
                    <h2>PC 연결 설정</h2>
                    <p className="path-text">앱 시작과 백그라운드 실행 관련 설정입니다.</p>
                  </div>
                </div>
                <AutostartSetting />
              </section>
              <FileEnginePanel
                embedded
                activeSection="settings"
                selectedRootId={selectedRootId}
                onSelectedRootIdChange={setSelectedRootId}
                hideRootPicker
              />
            </div>
            <div
              className="dashboard-view"
              hidden={section === "connection" || section === "settings"}
              aria-label="파일 관리 작업"
            >
              <FileEnginePanel
                embedded
                activeSection={section as FileSection}
                selectedRootId={selectedRootId}
                onSelectedRootIdChange={setSelectedRootId}
                hideRootPicker={section !== "rooms"}
              />
            </div>
          </div>
        </div>
      )}
    </main>
  );
}
