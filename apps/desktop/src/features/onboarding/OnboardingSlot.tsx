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
      <h1 id="onboarding-heading">Welcome to Housemouse</h1>

      {/* B's onboarding design / character intro plugs in here. */}
      <div className="onboarding-design-slot" data-slot="onboarding-design">
        <p>Pair your phone and pick a folder for Housemouse to help tidy.</p>
      </div>

      <ol className="onboarding-steps">
        <li>
          <span>Pair a device so your phone can send cleanup requests.</span>
          <button type="button" onClick={onFocusPairing} disabled={busy}>
            Go to pairing
          </button>
        </li>
        <li>
          <span>Register a folder as a managed root to get started.</span>
          <button type="button" onClick={() => void registerFolder()} disabled={busy}>
            {busy ? "Registering…" : "Register a folder"}
          </button>
        </li>
      </ol>

      {error ? <p className="error-text">{error}</p> : null}
    </section>
  );
}
