import { useEffect, useState } from "react";

import {
  CharacterEvent,
  CharacterEventKind,
  hideOverlay,
  listenForCharacterEvents,
  submitOverlayDraftRequest
} from "./overlayApi";

const KIND_LABELS: Record<CharacterEventKind, string> = {
  idle: "대기 중",
  analyzing: "분석 중",
  waiting_for_approval: "승인 대기 중",
  working: "작업 중",
  success: "완료",
  error: "확인 필요"
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
      setNotice(
        "AI 초안 흐름으로 보냈어요. 정리 결과는 실행 전에 승인해야 하는 제안으로만 만들어지며, 오버레이가 직접 파일을 바꾸지는 않아요."
      );
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
          숨기기
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
          aria-label="정리 초안 요청하기"
          placeholder="정리할 내용을 적어주세요 (초안만 만들며, 실행 전 승인이 필요해요)"
          maxLength={2000}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
        />
        <button type="submit" disabled={busy || draft.trim().length === 0}>
          초안 만들기
        </button>
      </form>
      {notice ? <p className="character-notice">{notice}</p> : null}
    </div>
  );
}
