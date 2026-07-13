import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/core/sync/realtime_controller.dart';
import 'package:mousekeeper/features/rooms/room_page.dart';

void main() {
  test('command realtime update patches only the matching command row', () {
    final original = [
      {'id': 'command-a', 'intent': 'ANALYZE', 'status': 'DELIVERED'},
      {'id': 'command-b', 'intent': 'MOVE', 'status': 'SUCCEEDED'},
    ];

    final patched = patchCommandItemsForRealtimeUpdate(
      commands: original,
      roomId: 'room-a',
      update: const RealtimeHomeUpdate(
        kind: RealtimeHomeUpdateKind.commandStatus,
        eventType: 'command.updated',
        roomId: 'room-a',
        commandId: 'command-a',
        commandStatus: 'ANALYZING',
      ),
    );

    expect(identical(patched, original), isFalse);
    expect(patched[0]['status'], 'ANALYZING');
    expect(identical(patched[1], original[1]), isTrue);
  });

  test('command realtime update upserts a partial command when unseen', () {
    final patched = patchCommandItemsForRealtimeUpdate(
      commands: const [],
      roomId: 'room-a',
      update: const RealtimeHomeUpdate(
        kind: RealtimeHomeUpdateKind.commandStatus,
        eventType: 'command.updated',
        roomId: 'room-a',
        commandId: 'command-a',
        commandStatus: 'FAILED',
      ),
    );

    expect(patched, [
      {'id': 'command-a', 'status': 'FAILED'},
    ]);
  });

  test('decision realtime update patches the related command row', () {
    final original = [
      {'id': 'command-a', 'intent': 'ANALYZE', 'status': 'WAITING_APPROVAL'},
    ];

    final patched = patchCommandItemsForRealtimeUpdate(
      commands: original,
      roomId: 'room-a',
      update: const RealtimeHomeUpdate(
        kind: RealtimeHomeUpdateKind.decisionCreated,
        eventType: 'decision.created',
        roomId: 'room-a',
        proposalId: 'proposal-a',
        commandId: 'command-a',
        proposalStatus: 'APPROVED',
        commandStatus: 'APPROVED',
        pendingProposalCount: 0,
      ),
    );

    expect(patched.single['status'], 'APPROVED');
  });

  test('execution realtime update patches only the matching execution row', () {
    final original = [
      {
        'execution': {'id': 'execution-a', 'status': 'EXECUTING'},
        'proposal': {'id': 'proposal-a'},
      },
      {
        'execution': {'id': 'execution-b', 'status': 'SUCCEEDED'},
        'proposal': {'id': 'proposal-b'},
      },
    ];

    final patched = patchExecutionItemsForRealtimeUpdate(
      executions: original,
      roomId: 'room-a',
      update: const RealtimeHomeUpdate(
        kind: RealtimeHomeUpdateKind.executionStatus,
        eventType: 'execution.updated',
        roomId: 'room-a',
        executionId: 'execution-a',
        executionStatus: 'FAILED',
      ),
    );

    expect(identical(patched, original), isFalse);
    expect((patched[0]['execution'] as Map)['status'], 'FAILED');
    expect(identical(patched[1], original[1]), isTrue);
  });

  test(
    'execution realtime update is a no-op for same status or other rooms',
    () {
      final original = [
        {
          'execution': {'id': 'execution-a', 'status': 'SUCCEEDED'},
        },
      ];

      expect(
        identical(
          patchExecutionItemsForRealtimeUpdate(
            executions: original,
            roomId: 'room-a',
            update: const RealtimeHomeUpdate(
              kind: RealtimeHomeUpdateKind.executionStatus,
              eventType: 'execution.updated',
              roomId: 'room-a',
              executionId: 'execution-a',
              executionStatus: 'SUCCEEDED',
            ),
          ),
          original,
        ),
        isTrue,
      );
      expect(
        identical(
          patchExecutionItemsForRealtimeUpdate(
            executions: original,
            roomId: 'room-a',
            update: const RealtimeHomeUpdate(
              kind: RealtimeHomeUpdateKind.executionStatus,
              eventType: 'execution.updated',
              roomId: 'room-b',
              executionId: 'execution-a',
              executionStatus: 'FAILED',
            ),
          ),
          original,
        ),
        isTrue,
      );
    },
  );

  test('execution realtime update upserts a partial execution when unseen', () {
    final patched = patchExecutionItemsForRealtimeUpdate(
      executions: const [],
      roomId: 'room-a',
      update: const RealtimeHomeUpdate(
        kind: RealtimeHomeUpdateKind.executionStatus,
        eventType: 'execution.updated',
        roomId: 'room-a',
        executionId: 'execution-a',
        executionStatus: 'EXECUTING',
      ),
    );

    expect(patched, [
      {
        'execution': {'id': 'execution-a', 'status': 'EXECUTING'},
        'proposal': null,
      },
    ]);
  });

  test('proposal realtime update patches only the matching proposal row', () {
    final original = [
      {
        'id': 'proposal-a',
        'status': 'OPEN',
        'summary': {'title': 'old'},
      },
      {'id': 'proposal-b', 'status': 'OPEN'},
    ];

    final patched = patchProposalItemsForRealtimeUpdate(
      proposals: original,
      roomId: 'room-a',
      update: const RealtimeHomeUpdate(
        kind: RealtimeHomeUpdateKind.proposalCreated,
        eventType: 'proposal.created',
        roomId: 'room-a',
        proposalId: 'proposal-a',
        commandId: 'command-a',
        proposalStatus: 'OPEN',
        proposalSummary: {'title': 'new'},
        proposalItemCount: 2,
        pendingProposalCount: 2,
      ),
    );

    expect(identical(patched, original), isFalse);
    expect((patched[0]['summary'] as Map)['title'], 'new');
    expect(patched[0]['commandId'], 'command-a');
    expect(patched[0]['itemCount'], 2);
    expect(identical(patched[1], original[1]), isTrue);
  });

  test('proposal realtime update upserts a partial proposal when unseen', () {
    final patched = patchProposalItemsForRealtimeUpdate(
      proposals: const [],
      roomId: 'room-a',
      update: const RealtimeHomeUpdate(
        kind: RealtimeHomeUpdateKind.proposalCreated,
        eventType: 'proposal.created',
        roomId: 'room-a',
        proposalId: 'proposal-a',
        commandId: 'command-a',
        proposalStatus: 'OPEN',
        proposalSummary: {'title': '정리 제안'},
        proposalItemCount: 3,
        pendingProposalCount: 1,
      ),
    );

    expect(patched, [
      {
        'id': 'proposal-a',
        'roomId': 'room-a',
        'status': 'OPEN',
        'commandId': 'command-a',
        'summary': {'title': '정리 제안'},
        'itemCount': 3,
      },
    ]);
  });

  test('decision realtime update removes a closed proposal from open list', () {
    final original = [
      {'id': 'proposal-a', 'status': 'OPEN'},
      {'id': 'proposal-b', 'status': 'OPEN'},
    ];

    final patched = patchProposalItemsForRealtimeUpdate(
      proposals: original,
      roomId: 'room-a',
      update: const RealtimeHomeUpdate(
        kind: RealtimeHomeUpdateKind.decisionCreated,
        eventType: 'decision.created',
        roomId: 'room-a',
        proposalId: 'proposal-a',
        commandId: 'command-a',
        decisionType: 'REJECT',
        proposalStatus: 'REJECTED',
        commandStatus: 'REJECTED',
        pendingProposalCount: 1,
      ),
    );

    expect(patched, [
      {'id': 'proposal-b', 'status': 'OPEN'},
    ]);
  });
}
