import { useCallback, useEffect, useState } from "react";

import { AgentPanel } from "./features/agent/AgentPanel";
import { listManagedRoots } from "./features/files/fileEngineApi";
import { FileEnginePanel } from "./features/files/FileEnginePanel";
import { OnboardingSlot } from "./features/onboarding/OnboardingSlot";
import { showOverlay } from "./features/overlay/overlayApi";

/**
 * Main-window shell. Routes the first-run, zero-managed-root state to the onboarding slot instead
 * of showing disabled file panels. Once at least one root exists (or when the root count can't be
 * read, e.g. outside the Tauri runtime), it shows the full agent + file panels.
 */
export function AppShell() {
  // null = unknown (loading or no Tauri runtime); we only gate on a confirmed zero.
  const [rootCount, setRootCount] = useState<number | null>(null);

  const reloadRootCount = useCallback(async () => {
    try {
      const roots = await listManagedRoots();
      setRootCount(roots.length);
    } catch {
      setRootCount(null);
    }
  }, []);

  useEffect(() => {
    void reloadRootCount();
    // Re-check when the window regains focus (e.g. after registering a folder via the native
    // picker) so the onboarding slot clears without a manual refresh.
    const onFocus = () => void reloadRootCount();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [reloadRootCount]);

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

  const isFirstRun = rootCount === 0;

  return (
    <main className="app-shell">
      <div className="app-shell-toolbar">
        <strong className="app-title">MOUSEKEEPER</strong>
        <button type="button" onClick={() => void openOverlay()}>
          캐릭터 오버레이 열기
        </button>
        {overlayError ? <span className="error-text">{overlayError}</span> : null}
      </div>
      {isFirstRun ? (
        <>
          <OnboardingSlot onRegistered={() => void reloadRootCount()} onFocusPairing={focusPairing} />
          <div id="agent-panel">
            <AgentPanel />
          </div>
        </>
      ) : (
        <>
          <div id="agent-panel">
            <AgentPanel />
          </div>
          <div id="file-engine-panel">
            <FileEnginePanel embedded />
          </div>
        </>
      )}
    </main>
  );
}
