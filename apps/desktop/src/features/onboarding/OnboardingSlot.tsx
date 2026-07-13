import { useState } from "react";

import {
  registerManagedRoot,
  selectManagedRootDirectory
} from "../files/fileEngineApi";

/**
 * A-owned first-run slot, shown when no managed root exists yet. A provides the empty slot and the
 * safe hand-offs; B replaces the copy/illustration inside `onboarding-design-slot` with the real
 * onboarding + character design.
 *
 * Hand-offs never bypass safety: "Register a folder" goes through the same `register_managed_root`
 * command the file panel uses (path guard, overlap checks), and pairing is delegated to the agent
 * panel. This slot performs no direct file mutation of its own.
 */
export function OnboardingSlot({
  onRegistered,
  onFocusPairing
}: {
  onRegistered: () => void;
  onFocusPairing: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function registerFolder() {
    setError(null);
    setBusy(true);
    try {
      const path = await selectManagedRootDirectory();
      if (!path) return;
      await registerManagedRoot(path);
      onRegistered();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="onboarding-slot" aria-labelledby="onboarding-heading">
      <h1 id="onboarding-heading">MOUSEKEEPER에 오신 걸 환영해요</h1>

      {/* B's onboarding design / character intro plugs in here. */}
      <div className="onboarding-design-slot" data-slot="onboarding-design">
        <p>휴대폰을 페어링하고, MOUSEKEEPER가 정리를 도울 폴더를 골라주세요.</p>
      </div>

      <ol className="onboarding-steps">
        <li>
          <span>휴대폰에서 정리를 요청할 수 있도록 기기를 페어링하세요.</span>
          <button type="button" onClick={onFocusPairing} disabled={busy}>
            페어링하러 가기
          </button>
        </li>
        <li>
          <span>정리할 폴더를 관리 폴더로 등록하면 시작할 수 있어요.</span>
          <button type="button" onClick={() => void registerFolder()} disabled={busy}>
            {busy ? "등록 중…" : "폴더 등록하기"}
          </button>
        </li>
      </ol>

      {error ? <p className="error-text">{error}</p> : null}
    </section>
  );
}
