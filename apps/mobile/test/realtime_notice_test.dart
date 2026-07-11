import 'package:flutter_test/flutter_test.dart';
import 'package:housemouse/core/sync/realtime_controller.dart';

void main() {
  test('live proposal and terminal execution events create user notices', () {
    final proposal = realtimeNoticeFor('proposal.created', {
      'eventId': 'proposal-event',
      'payload': {'proposalId': 'proposal-id'},
    });
    final stale = realtimeNoticeFor('execution.updated', {
      'eventId': 'execution-event',
      'payload': {'status': 'STALE'},
    });
    expect(proposal?.message, contains('승인 전에'));
    expect(stale?.message, contains('파일이 변경'));
  });

  test('non-terminal or unrelated events do not invent notifications', () {
    expect(
      realtimeNoticeFor('execution.updated', {
        'eventId': 'working-event',
        'payload': {'status': 'EXECUTING'},
      }),
      isNull,
    );
    expect(
      realtimeNoticeFor('presence.updated', {
        'eventId': 'presence-event',
        'payload': {'presence': 'ONLINE_IDLE'},
      }),
      isNull,
    );
  });
}
