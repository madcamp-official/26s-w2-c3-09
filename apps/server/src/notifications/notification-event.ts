export type NotificationContent = { title: string; body: string };

const executionBodies: Record<string, string> = {
  SUCCEEDED: '정리 작업을 완료했어요.',
  PARTIALLY_SUCCEEDED: '일부 파일만 정리됐어요. 결과를 확인해 주세요.',
  FAILED: '정리 작업을 완료하지 못했어요. 결과를 확인해 주세요.',
  STALE: '파일이 변경되어 정리 작업을 멈췄어요.',
  ROLLED_BACK: '정리 작업을 되돌렸어요.',
};

export function notificationForEvent(
  eventType: string,
  payload: Record<string, unknown>,
): NotificationContent | null {
  if (eventType === 'proposal.created') {
    return {
      title: '집쥐가 정리안을 만들었어요',
      body: '휴대폰에서 내용을 확인하고 승인해 주세요.',
    };
  }
  if (eventType === 'execution.updated') {
    const status = typeof payload.status === 'string' ? payload.status : '';
    const body = executionBodies[status];
    return body ? { title: '정리 작업 결과', body } : null;
  }
  if (eventType === 'file.transfer.updated' && payload.status === 'READY') {
    return {
      title: '파일을 받을 수 있어요',
      body: '요청한 파일의 다운로드 준비가 끝났어요.',
    };
  }
  return null;
}
