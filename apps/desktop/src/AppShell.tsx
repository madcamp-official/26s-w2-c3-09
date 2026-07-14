import { useCallback, useEffect, useState } from "react";

import { getAgentConnectionStatus } from "./features/agent/agentApi";
import { AgentPanel } from "./features/agent/AgentPanel";
import { listManagedRoots } from "./features/files/fileEngineApi";
import { FileEnginePanel } from "./features/files/FileEnginePanel";
import { OnboardingSlot } from "./features/onboarding/OnboardingSlot";
import { setHouseOverlayLocked, showOverlay } from "./features/overlay/overlayApi";

const splashUrl = new URL("./assets/mousekeeper-splash.png", import.meta.url).href;
const setupMascotUrl = new URL(
  "../../../packages/character-assets/new_mouse/mouse_walk_clean.png",
  import.meta.url
).href;

type DashboardSection = "connection" | "organize" | "explore" | "history";
type FileSection = Exclude<DashboardSection, "connection">;

const DASHBOARD_SECTIONS: ReadonlyArray<{
  id: DashboardSection;
  icon: string;
  label: string;
  hint: string;
}> = [
  { id: "organize", icon: "🧹", label: "폴더 정리", hint: "분석 · 제안 · 실행" },
  { id: "explore", icon: "🔍", label: "검색 · 탐색", hint: "파일 찾기 · 열기" },
  { id: "history", icon: "🗂️", label: "작업 기록", hint: "결과 · 되돌리기" },
  { id: "connection", icon: "🔌", label: "PC 연결", hint: "페어링 · 백그라운드" }
];

/**
 * Main-window shell. The setup screen stays visible until both halves are ready: one paired
 * desktop device and at least one managed folder. That keeps users from registering a folder,
 * pairing later, then wondering why they need to register the same folder again.
 */
export function AppShell() {
  // null = unknown (loading or no Tauri runtime); confirmed values drive the setup gate.
  const [rootCount, setRootCount] = useState<number | null>(null);
  const [connectionState, setConnectionState] = useState<string | null>(null);
  const [section, setSection] = useState<DashboardSection>("organize");

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

  const isPaired =
    connectionState === null
      ? null
      : !["unconfigured", "revoked"].includes(connectionState);

  useEffect(() => {
    if (rootCount !== 0 && isPaired !== false) return;
    const timer = window.setInterval(() => void refreshSetupState(), 2_000);
    return () => window.clearInterval(timer);
  }, [isPaired, refreshSetupState, rootCount]);

  const focusPairing = useCallback(() => {
    document.getElementById("agent-panel")?.scrollIntoView({ behavior: "smooth" });
  }, []);

  const [overlayError, setOverlayError] = useState<string | null>(null);
  const openOverlay = useCallback(async () => {
    setOverlayError(null);
    try {
      await showOverlay();
    } catch (cause) {
      setOverlayError(cause instanceof Error ? cause.message : String(cause));
    }
  }, []);

  const unlockHouseOverlay = useCallback(async () => {
    setOverlayError(null);
    try {
      await setHouseOverlayLocked(false);
    } catch (cause) {
      setOverlayError(cause instanceof Error ? cause.message : String(cause));
    }
  }, []);

  const hasFolder = rootCount !== null && rootCount > 0;
  const needsSetup = rootCount === null || isPaired === null || rootCount === 0 || isPaired === false;

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
            {isPaired ? "모바일 연결됨" : "페어링 필요"}
          </span>
          <button type="button" onClick={() => void openOverlay()}>
            마스코트 열기
          </button>
          <button type="button" onClick={() => void unlockHouseOverlay()}>
            집 위치 조정
          </button>
        </div>
        {overlayError ? <span className="error-text">{overlayError}</span> : null}
      </div>

      {needsSetup ? (
        <div className="setup-flow">
          <section className="manager-hero" aria-labelledby="manager-hero-title">
            <div className="manager-hero-art">
              <img src={splashUrl} alt="MOUSEKEEPER" />
            </div>
            <div className="manager-hero-copy">
              <span className="setup-kicker">초기 설정</span>
              <h1 id="manager-hero-title">PC 방을 생쥐 매니저에게 맡길 준비를 해요</h1>
              <p>
                모바일 앱과 PC를 연결하고, 정리할 폴더를 한 번만 등록하면 됩니다. 순서가
                바뀌어도 괜찮고, 등록된 폴더는 페어링 뒤 자동으로 모바일 방과 이어집니다.
              </p>
              <div className="setup-checklist" aria-label="설치 진행 상태">
                <span className={isPaired ? "is-done" : ""}>1. 모바일 페어링</span>
                <span className={hasFolder ? "is-done" : ""}>2. 관리 폴더 등록</span>
                <span className={isPaired && hasFolder ? "is-done" : ""}>3. 관리 시작</span>
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
                  <p>QR 또는 6자리 코드로 이 PC를 계정에 연결합니다.</p>
                </div>
              </div>
              <AgentPanel />
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
                <small>안전하게 정리할 준비 완료</small>
              </div>
            </div>
            <nav className="dashboard-nav">
              {DASHBOARD_SECTIONS.map((item) => (
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
              <AgentPanel />
            </div>
            <div
              className="dashboard-view"
              hidden={section === "connection"}
              aria-label="파일 관리 작업대"
            >
              <FileEnginePanel
                embedded
                activeSection={(section === "connection" ? "organize" : section) as FileSection}
              />
            </div>
          </div>
        </div>
      )}
    </main>
  );
}
