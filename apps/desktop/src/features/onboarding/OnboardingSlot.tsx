import { useState } from "react";

import {
  registerManagedRoot,
  selectManagedRootDirectory
} from "../files/fileEngineApi";

/**
 * A-owned first-run folder slot. It only registers an explicit managed root through the same
 * guarded command used by the file panel; it never mutates files or bypasses approval.
 */
export function OnboardingSlot({
  hasFolder,
  onRegistered,
  onFocusPairing
}: {
  hasFolder: boolean;
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
    <section className="onboarding-slot" aria-label="관리 폴더 등록">
      <div className="onboarding-design-slot" data-slot="onboarding-design">
        <strong>{hasFolder ? "폴더 등록 완료" : "정리할 폴더를 골라주세요"}</strong>
        <p>
          등록한 폴더 안에서만 MouseKeeper가 분석과 정리 제안을 만듭니다. 실제 이동이나 삭제는
          승인, 사전 검사, 작업 기록을 거친 뒤에만 실행됩니다.
        </p>
      </div>

      <div className="onboarding-actions">
        <button type="button" className="primary-button" onClick={() => void registerFolder()} disabled={busy}>
          {busy ? "폴더 확인 중" : hasFolder ? "다른 폴더 추가" : "관리 폴더 선택"}
        </button>
        <button type="button" onClick={onFocusPairing} disabled={busy}>
          페어링 카드로 이동
        </button>
      </div>

      <p className="mode-note">
        폴더를 먼저 등록해도 괜찮습니다. 모바일 페어링이 끝나면 등록된 폴더가 자동으로 방과 연결됩니다.
      </p>
      {error ? <p className="error-text">{error}</p> : null}
    </section>
  );
}
