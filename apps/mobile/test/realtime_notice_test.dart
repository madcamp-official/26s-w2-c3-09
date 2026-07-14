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
    final paired = realtimeHomeUpdateFor('device.paired', {
      'eventId': 'paired-event',
      'eventType': 'device.paired',
      'payload': {
        'deviceId': 'device-a',
        'status': 'ACTIVE',
        'device': {
          'id': 'device-a',
          'platform': 'WINDOWS',
          'deviceName': 'Desktop',
          'status': 'ACTIVE',
          'lastSeenAt': null,
          'createdAt': '2026-07-13T01:02:03.000Z',
        },
      },
    });
    final legacyPaired = realtimeHomeUpdateFor('device.paired', {
      'eventId': 'legacy-paired-event',
      'eventType': 'device.paired',
      'payload': {'deviceId': 'device-b', 'status': 'ACTIVE'},
    });

    expect(execution?.kind, RealtimeHomeUpdateKind.executionStatus);
    expect(execution?.roomId, 'room-a');
    expect(execution?.executionId, 'execution-a');
    expect(execution?.executionStatus, 'SUCCEEDED');
    expect(paired?.kind, RealtimeHomeUpdateKind.devicePaired);
    expect(paired?.device?['deviceName'], 'Desktop');
    expect(legacyPaired?.kind, RealtimeHomeUpdateKind.refreshSummary);
  });

  test('file transfer updates become transfer-scoped realtime patches', () {
    final ready = realtimeFileTransferUpdateFor('file.transfer.updated', {
      'eventId': 'transfer-event',
      'eventType': 'file.transfer.updated',
      'aggregateId': 'transfer-a',
      'roomId': 'room-a',
      'payload': {
        'transferId': 'transfer-a',
        'roomId': 'room-a',
        'status': 'READY',
      },
    });
    final failed = realtimeFileTransferUpdateFor('file.transfer.updated', {
      'eventId': 'transfer-event',
      'eventType': 'file.transfer.updated',
      'aggregateId': 'transfer-b',
      'payload': {
        'transferId': 'transfer-b',
        'status': 'FAILED',
        'failureCode': 'SOURCE_CHANGED',
      },
    });

    expect(ready?.transferId, 'transfer-a');
    expect(ready?.roomId, 'room-a');
    expect(ready?.status, 'READY');
    expect(failed?.failureCode, 'SOURCE_CHANGED');
    expect(
      realtimeFileTransferUpdateFor('file.transfer.updated', {
        'payload': {'transferId': 'transfer-c', 'status': 'MADE_UP'},
      }),
      isNull,
    );
  });

  test(
    'file browse terminal events become request-scoped realtime patches',
    () {
      final ready = realtimeFileBrowseUpdateFor('file.browse.ready', {
        'eventId': 'browse-event',
        'eventType': 'file.browse.ready',
        'aggregateId': 'browse-a',
        'roomId': 'room-a',
        'payload': {
          'requestId': 'browse-a',
          'roomId': 'room-a',
          'status': 'READY',
        },
      });
      final failed = realtimeFileBrowseUpdateFor('file.browse.failed', {
        'eventId': 'browse-event',
        'eventType': 'file.browse.failed',
        'aggregateId': 'browse-b',
        'payload': {'requestId': 'browse-b', 'failureCode': 'TIMED_OUT'},
      });

      expect(ready?.requestId, 'browse-a');
      expect(ready?.roomId, 'room-a');
      expect(ready?.status, 'READY');
      expect(failed?.status, 'FAILED');
      expect(failed?.failureCode, 'TIMED_OUT');
      expect(
        realtimeHomeUpdateFor('file.browse.ready', {
          'eventId': 'browse-event',
          'eventType': 'file.browse.ready',
          'aggregateId': 'browse-a',
        }),
        isNull,
      );
    },
  );

  test('file directory events become directory-scoped realtime patches', () {
    final update = realtimeFileDirectoryUpdateFor('file.directory.updated', {
      'eventId': 'directory-event',
      'eventType': 'file.directory.updated',
      'roomId': 'room-a',
      'payload': {
        'roomId': 'room-a',
        'kind': 'FILE_MOVED',
        'parentRelativePath': 'reports',
        'previousRelativePath': 'reports/old.pdf',
        'entry': {
          'type': 'FILE',
          'name': 'final.pdf',
          'relativePath': 'reports/final.pdf',
        },
      },
    });

    expect(update?.roomId, 'room-a');
    expect(update?.kind, 'FILE_MOVED');
    expect(update?.previousRelativePath, 'reports/old.pdf');
    expect(update?.entry?['relativePath'], 'reports/final.pdf');
    expect(
      realtimeFileDirectoryUpdateFor('file.directory.updated', {
        'payload': {'roomId': 'room-a', 'kind': 'UNKNOWN'},
      }),
      isNull,
    );
  });

  test('chat message events become session-scoped realtime patches', () {
    final update = realtimeChatMessageUpdateFor('chat.message.created', {
      'eventId': 'chat-event',
      'eventType': 'chat.message.created',
      'aggregateId': 'message-a',
      'roomId': 'room-a',
      'payload': {
        'messageId': 'message-a',
        'sessionId': 'session-a',
        'senderType': 'ASSISTANT',
        'messageType': 'TEXT',
      },
    });

    expect(update?.messageId, 'message-a');
    expect(update?.sessionId, 'session-a');
    expect(update?.roomId, 'room-a');
    expect(update?.senderType, 'ASSISTANT');
    expect(update?.messageType, 'TEXT');
    expect(
      realtimeChatMessageUpdateFor('chat.message.created', {
        'payload': {'messageId': 'message-a'},
      }),
      isNull,
    );
  });

  test('complete realtime patches suppress generic page revision fan-out', () {
    final presence = realtimeHomeUpdateFor('presence.updated', {
      'deviceId': 'device-a',
      'presence': 'ONLINE_IDLE',
    });
    final proposal = realtimeHomeUpdateFor('proposal.created', {
      'eventId': 'proposal-event',
      'eventType': 'proposal.created',
      'aggregateId': 'proposal-a',
      'roomId': 'room-a',
      'payload': {
        'proposalId': 'proposal-a',
        'roomId': 'room-a',
        'status': 'OPEN',
        'pendingProposalCount': 1,
      },
    });
    final decision = realtimeHomeUpdateFor('decision.created', {
      'eventId': 'decision-event',
      'eventType': 'decision.created',
      'aggregateId': 'decision-a',
      'roomId': 'room-a',
      'payload': {
        'proposalId': 'proposal-a',
        'roomId': 'room-a',
        'proposalStatus': 'APPROVED',
        'pendingProposalCount': 0,
      },
    });
    final snapshot = realtimeHomeUpdateFor('room.snapshot.updated', {
      'eventId': 'snapshot-event',
      'eventType': 'room.snapshot.updated',
      'aggregateId': 'snapshot-a',
      'roomId': 'room-a',
      'payload': {
        'snapshotId': 'snapshot-a',
        'roomId': 'room-a',
        'score': 88,
        'metrics': {'deductions': []},
        'formulaVersion': 'mousekeeper-cleanliness-v1',
        'calculatedAt': '2026-07-13T00:00:00.000Z',
      },
    });
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
    final execution = realtimeHomeUpdateFor('execution.updated', {
      'eventId': 'execution-event',
      'eventType': 'execution.updated',
      'aggregateId': 'execution-a',
      'roomId': 'room-a',
      'payload': {
        'executionId': 'execution-a',
        'roomId': 'room-a',
        'status': 'SUCCEEDED',
      },
    });
    final deviceRemoved = realtimeHomeUpdateFor('device.revoked', {
      'payload': {'deviceId': 'device-a'},
    });
    final devicePaired = realtimeHomeUpdateFor('device.paired', {
      'payload': {
        'deviceId': 'device-b',
        'device': {
          'id': 'device-b',
          'platform': 'WINDOWS',
          'deviceName': 'Second desktop',
          'status': 'ACTIVE',
          'lastSeenAt': null,
          'createdAt': '2026-07-13T01:02:03.000Z',
        },
      },
    });
    final roomRemoved = realtimeHomeUpdateFor('room.removed', {
      'payload': {'roomId': 'room-a'},
    });
    final fileTransfer = realtimeFileTransferUpdateFor(
      'file.transfer.updated',
      {
        'payload': {'transferId': 'transfer-a', 'status': 'READY'},
      },
    );
    final fileBrowse = realtimeFileBrowseUpdateFor('file.browse.ready', {
      'payload': {'requestId': 'browse-a', 'status': 'READY'},
    });
    final fileDirectory = realtimeFileDirectoryUpdateFor(
      'file.directory.updated',
      {
        'payload': {
          'roomId': 'room-a',
          'kind': 'FILE_UPDATED',
          'parentRelativePath': 'reports',
          'entry': {'relativePath': 'reports/a.pdf', 'name': 'a.pdf'},
        },
      },
    );
    final chatMessage = realtimeChatMessageUpdateFor('chat.message.created', {
      'payload': {'messageId': 'message-a', 'sessionId': 'session-a'},
    });

    for (final entry in <(String, RealtimeHomeUpdate?)>[
      ('presence.updated', presence),
      ('proposal.created', proposal),
      ('decision.created', decision),
      ('room.snapshot.updated', snapshot),
      ('command.updated', command),
      ('execution.updated', execution),
      ('device.paired', devicePaired),
      ('device.revoked', deviceRemoved),
      ('room.removed', roomRemoved),
    ]) {
      expect(realtimeUpdateSuppressesGenericRevision(entry.$1, entry.$2), true);
    }
    expect(
      realtimeUpdateSuppressesGenericRevision('presence.updated', null),
      true,
    );
    expect(
      realtimeUpdateSuppressesGenericRevision(
        'file.transfer.updated',
        null,
        fileTransfer,
      ),
      true,
    );
    expect(
      realtimeUpdateSuppressesGenericRevision(
        'file.browse.ready',
        null,
        null,
        fileBrowse,
      ),
      true,
    );
    expect(
      realtimeUpdateSuppressesGenericRevision('file.browse.failed', null),
      true,
    );
    expect(
      realtimeUpdateSuppressesGenericRevision(
        'file.directory.updated',
        null,
        null,
        null,
        fileDirectory,
      ),
      true,
    );
    expect(
      realtimeUpdateSuppressesGenericRevision(
        'chat.message.created',
        null,
        null,
        null,
        null,
        chatMessage,
      ),
      true,
    );
  });

  test(
    'summary fallback and unknown events keep generic revision available',
    () {
      final summaryFallback = realtimeHomeUpdateFor('proposal.created', {
        'eventId': 'proposal-event',
        'eventType': 'proposal.created',
        'payload': {'proposalId': 'proposal-a'},
      });

      expect(summaryFallback?.kind, RealtimeHomeUpdateKind.refreshSummary);
      expect(
        realtimeUpdateSuppressesGenericRevision(
          'proposal.created',
          summaryFallback,
        ),
        false,
      );
      expect(
        realtimeUpdateSuppressesGenericRevision('command.available', null),
        false,
      );
    },
  );

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

  test('decision created becomes a room-scoped proposal and command patch', () {
    final decision = realtimeHomeUpdateFor('decision.created', {
      'eventId': 'decision-event',
      'eventType': 'decision.created',
      'aggregateId': 'decision-a',
      'roomId': 'room-a',
      'payload': {
        'decisionId': 'decision-a',
        'proposalId': 'proposal-a',
        'roomId': 'room-a',
        'commandId': 'command-a',
        'decisionType': 'APPROVE',
        'proposalStatus': 'APPROVED',
        'commandStatus': 'APPROVED',
        'pendingProposalCount': 0,
      },
    });

    expect(decision?.kind, RealtimeHomeUpdateKind.decisionCreated);
    expect(decision?.decisionId, 'decision-a');
    expect(decision?.proposalId, 'proposal-a');
    expect(decision?.roomId, 'room-a');
    expect(decision?.commandId, 'command-a');
    expect(decision?.decisionType, 'APPROVE');
    expect(decision?.proposalStatus, 'APPROVED');
    expect(decision?.commandStatus, 'APPROVED');
    expect(decision?.pendingProposalCount, 0);
  });

  test('incomplete decision created events request one summary fallback', () {
    final decision = realtimeHomeUpdateFor('decision.created', {
      'eventId': 'decision-event',
      'eventType': 'decision.created',
      'aggregateId': 'decision-a',
      'payload': {'proposalId': 'proposal-a'},
    });

    expect(decision?.kind, RealtimeHomeUpdateKind.refreshSummary);
  });

  test('room snapshot updated becomes a room-scoped cleanliness patch', () {
    final snapshot = realtimeHomeUpdateFor('room.snapshot.updated', {
      'eventId': 'snapshot-event',
      'eventType': 'room.snapshot.updated',
      'aggregateId': 'snapshot-a',
      'roomId': 'room-a',
      'payload': {
        'snapshotId': 'snapshot-a',
        'roomId': 'room-a',
        'score': 88,
        'metrics': {
          'totalFileCount': 10,
          'managedFileCount': 8,
          'unorganizedFileCount': 2,
          'deductions': [],
        },
        'formulaVersion': 'mousekeeper-cleanliness-v1',
        'calculatedAt': '2026-07-13T00:00:00.000Z',
      },
    });

    expect(snapshot?.kind, RealtimeHomeUpdateKind.roomSnapshotUpdated);
    expect(snapshot?.snapshotId, 'snapshot-a');
    expect(snapshot?.roomId, 'room-a');
    expect(snapshot?.roomSnapshot?['score'], 88);
    expect(
      snapshot?.roomSnapshot?['formulaVersion'],
      'mousekeeper-cleanliness-v1',
    );
  });

  test('incomplete room snapshot events request one summary fallback', () {
    final snapshot = realtimeHomeUpdateFor('room.snapshot.updated', {
      'eventId': 'snapshot-event',
      'eventType': 'room.snapshot.updated',
      'aggregateId': 'snapshot-a',
      'payload': {'snapshotId': 'snapshot-a'},
    });

    expect(snapshot?.kind, RealtimeHomeUpdateKind.refreshSummary);
  });
}
