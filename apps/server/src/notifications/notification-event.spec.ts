import { notificationForEvent } from './notification-event';

describe('notificationForEvent', () => {
  it('creates approval and terminal execution notifications', () => {
    expect(notificationForEvent('proposal.created', {})).not.toBeNull();
    expect(
      notificationForEvent('execution.updated', { status: 'SUCCEEDED' }),
    ).not.toBeNull();
    expect(
      notificationForEvent('execution.updated', { status: 'EXECUTING' }),
    ).toBeNull();
  });

  it('notifies only when a file transfer is ready', () => {
    expect(
      notificationForEvent('file.transfer.updated', { status: 'READY' }),
    ).not.toBeNull();
    expect(
      notificationForEvent('file.transfer.updated', { status: 'UPLOADING' }),
    ).toBeNull();
    expect(notificationForEvent('unrelated.event', {})).toBeNull();
  });
});
