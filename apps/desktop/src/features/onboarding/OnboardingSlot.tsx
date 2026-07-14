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
        폴더만 등록해도 PC의 로컬 관리 기능을 바로 사용할 수 있습니다. 모바일이 필요하면 이후
        PC 연결 탭에서 페어링하고, 방 관리에서 폴더를 동기화하세요.
      </p>
      {error ? <p className="error-text">{error}</p> : null}
    </section>
  );
}
