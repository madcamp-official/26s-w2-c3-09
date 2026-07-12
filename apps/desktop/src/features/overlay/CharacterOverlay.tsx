import { useEffect, useState } from "react";

import {
  CharacterEvent,
  CharacterEventKind,
  hideOverlay,
  listenForCharacterEvents,
  submitOverlayDraftRequest
} from "./overlayApi";

const KIND_LABELS: Record<CharacterEventKind, string> = {
  idle: "Idle",
  analyzing: "Analyzing",
  waiting_for_approval: "Waiting for approval",
  working: "Working",
  success: "Success",
  error: "Attention needed"
};

/**
 * A-owned overlay shell. It renders the current character *state* (driven by CharacterEvents from
 * the background bridge) and a chat input that can only hand a draft request to the app. The inner
 * `character-stage` element is the stable mount point B plugs the real character design/motion into.
 *
 * This component never imports the file-engine API: the overlay cannot call a direct file
 * operation or bypass approval / precheck / journal / execution.
 */
export function CharacterOverlay() {
  const [event, setEvent] = useState<CharacterEvent>({ kind: "idle" });
  const [draft, setDraft] = useState("");
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const unlisten = listenForCharacterEvents(setEvent);
    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

  async function submitDraft() {
    setNotice(null);
    setBusy(true);
    try {
      await submitOverlayDraftRequest(draft);
      setDraft("");
      setNotice("Draft handed off to the proposal flow. It still needs your approval before anything runs.");
    } catch (cause) {
      setNotice(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className={`character-overlay character-${event.kind}`}>
      <header className="character-overlay-header">
        <span className="character-state-badge">{KIND_LABELS[event.kind]}</span>
        <button className="character-overlay-hide" onClick={() => void hideOverlay()}>
          Hide
        </button>
      </header>

      {/* B plugs the character design / Rive animation into this stage. A only owns the shell. */}
      <div className="character-stage" data-character-kind={event.kind} aria-live="polite">
        <div className="character-placeholder">🐭</div>
        {event.message ? <p className="character-message">{event.message}</p> : null}
      </div>

      <form
        className="character-chat"
        onSubmit={(e) => {
          e.preventDefault();
          void submitDraft();
        }}
      >
        <input
          aria-label="Ask the agent to draft a cleanup"
          placeholder="Describe a cleanup (draft only — you approve before it runs)"
          maxLength={2000}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
        />
        <button type="submit" disabled={busy || draft.trim().length === 0}>
          Draft
        </button>
      </form>
      {notice ? <p className="character-notice">{notice}</p> : null}
    </div>
  );
}
