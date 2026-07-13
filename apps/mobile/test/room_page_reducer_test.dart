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
}
