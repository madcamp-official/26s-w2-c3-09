import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/core/models/character_state.dart';
import 'package:mousekeeper/core/sync/realtime_controller.dart';

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

  test('validated realtime character events preserve the server state', () {
    expect(
      realtimeCharacterKindFor('character.event', {'kind': 'SUCCESS'}),
      CharacterState.success,
    );
    expect(
      realtimeCharacterKindFor('character.event', {'kind': 'MADE_UP'}),
      isNull,
    );
    expect(
      realtimeCharacterKindFor('presence.updated', {
        'payload': {'presence': 'OFFLINE'},
      }),
      CharacterState.offline,
    );
    expect(
      realtimeCharacterKindFor('presence.updated', {
        'deviceId': 'device-a',
        'presence': 'ONLINE_EXECUTING',
        'ttlSeconds': 15,
      }),
      CharacterState.working,
    );
  });

  test('raw presence payload becomes a device-scoped home patch', () {
    final update = realtimeHomeUpdateFor('presence.updated', {
      'deviceId': 'device-a',
      'presence': 'ONLINE_SCANNING',
      'ttlSeconds': 15,
    });

    expect(update?.kind, RealtimeHomeUpdateKind.presence);
    expect(update?.deviceId, 'device-a');
    expect(update?.presence, 'ONLINE_SCANNING');
    expect(
      realtimeHomeUpdateFor('presence.updated', {
        'deviceId': 'device-a',
        'presence': 'INVENTED_STATE',
      }),
      isNull,
    );
  });

  test('domain events use a targeted patch or one summary fallback', () {
    final execution = realtimeHomeUpdateFor('execution.updated', {
      'eventId': 'execution-event',
      'eventType': 'execution.updated',
      'roomId': 'room-a',
      'aggregateId': 'execution-a',
      'payload': {
        'executionId': 'execution-a',
        'roomId': 'room-a',
        'status': 'SUCCEEDED',
      },
    });
    final snapshot = realtimeHomeUpdateFor('room.snapshot.updated', {
      'eventId': 'snapshot-event',
      'eventType': 'room.snapshot.updated',
      'roomId': 'room-a',
      'payload': {'snapshotId': 'snapshot-a'},
    });

    expect(execution?.kind, RealtimeHomeUpdateKind.executionStatus);
    expect(execution?.roomId, 'room-a');
    expect(execution?.executionId, 'execution-a');
    expect(execution?.executionStatus, 'SUCCEEDED');
    expect(snapshot?.kind, RealtimeHomeUpdateKind.refreshSummary);
  });

  test('execution updated falls back to envelope aggregate id', () {
    final execution = realtimeHomeUpdateFor('execution.updated', {
      'eventId': 'execution-event',
      'eventType': 'execution.updated',
      'aggregateId': 'execution-from-envelope',
      'roomId': 'room-a',
      'payload': {'status': 'FAILED'},
    });

    expect(execution?.kind, RealtimeHomeUpdateKind.executionStatus);
    expect(execution?.executionId, 'execution-from-envelope');
    expect(execution?.roomId, 'room-a');
    expect(execution?.executionStatus, 'FAILED');
  });

  test('command updated becomes a room-scoped command status patch', () {
    final command = realtimeHomeUpdateFor('command.updated', {
      'eventId': 'command-event',
      'eventType': 'command.updated',
      'aggregateId': 'command-a',
      'roomId': 'room-a',
      'payload': {
        'commandId': 'command-a',
        'roomId': 'room-a',
        'status': 'ANALYZING',
      },
    });

    expect(command?.kind, RealtimeHomeUpdateKind.commandStatus);
    expect(command?.commandId, 'command-a');
    expect(command?.roomId, 'room-a');
    expect(command?.commandStatus, 'ANALYZING');
  });

  test('proposal created becomes a room-scoped proposal patch', () {
    final proposal = realtimeHomeUpdateFor('proposal.created', {
      'eventId': 'proposal-event',
      'eventType': 'proposal.created',
      'aggregateId': 'proposal-a',
      'roomId': 'room-a',
      'payload': {
        'proposalId': 'proposal-a',
        'roomId': 'room-a',
        'commandId': 'command-a',
        'status': 'OPEN',
        'summary': {'title': '정리 제안'},
        'itemCount': 2,
        'pendingProposalCount': 3,
      },
    });

    expect(proposal?.kind, RealtimeHomeUpdateKind.proposalCreated);
    expect(proposal?.proposalId, 'proposal-a');
    expect(proposal?.roomId, 'room-a');
    expect(proposal?.commandId, 'command-a');
    expect(proposal?.proposalStatus, 'OPEN');
    expect(proposal?.proposalSummary?['title'], '정리 제안');
    expect(proposal?.proposalItemCount, 2);
    expect(proposal?.pendingProposalCount, 3);
  });

  test('incomplete proposal created events request one summary fallback', () {
    final proposal = realtimeHomeUpdateFor('proposal.created', {
      'eventId': 'proposal-event',
      'eventType': 'proposal.created',
      'aggregateId': 'proposal-a',
      'payload': {'commandId': 'command-a'},
    });

    expect(proposal?.kind, RealtimeHomeUpdateKind.refreshSummary);
  });
}
