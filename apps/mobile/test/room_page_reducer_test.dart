import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/core/sync/realtime_controller.dart';
import 'package:mousekeeper/features/rooms/room_page.dart';

void main() {
  test('room read providers request split room projections', () async {
    final listPaths = <String>[];
    final nullablePaths = <String>[];
    final container = ProviderContainer(
      overrides: [
        roomListFetcherProvider.overrideWithValue((path) async {
          listPaths.add(path);
          return [
            {'path': path},
          ];
        }),
        roomNullableFetcherProvider.overrideWithValue((path) async {
          nullablePaths.add(path);
          return {'path': path};
        }),
      ],
    );
    addTearDown(container.dispose);

    await Future.wait([
      container.read(roomCommandListProvider('room-a').future),
      container.read(roomProposalListProvider('room-a').future),
      container.read(roomExecutionListProvider('room-a').future),
      container.read(roomActivityListProvider('room-a').future),
    ]);
    final snapshot = await container.read(
      roomSnapshotProvider('room-a').future,
    );

    expect(listPaths, [
      '/v1/rooms/room-a/commands',
      '/v1/rooms/room-a/proposals/open',
      '/v1/rooms/room-a/executions',
      '/v1/rooms/room-a/activity?limit=20',
    ]);
    expect(nullablePaths, ['/v1/rooms/room-a/snapshots/latest']);
    expect(snapshot, {'path': '/v1/rooms/room-a/snapshots/latest'});
  });

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

  test(
    'room snapshot realtime update replaces only newer cleanliness data',
    () {
      final original = {
        'id': 'snapshot-old',
        'roomId': 'room-a',
        'score': 52,
        'metrics': {'deductions': []},
        'formulaVersion': 'mousekeeper-cleanliness-v1',
        'calculatedAt': '2026-07-13T09:00:00.000Z',
      };

      final patched = patchRoomSnapshotForRealtimeUpdate(
        snapshot: original,
        roomId: 'room-a',
        update: const RealtimeHomeUpdate(
          kind: RealtimeHomeUpdateKind.roomSnapshotUpdated,
          eventType: 'room.snapshot.updated',
          roomId: 'room-a',
          snapshotId: 'snapshot-new',
          roomSnapshot: {
            'id': 'snapshot-new',
            'roomId': 'room-a',
            'score': 88,
            'metrics': {'deductions': []},
            'formulaVersion': 'mousekeeper-cleanliness-v1',
            'calculatedAt': '2026-07-13T10:00:00.000Z',
          },
        ),
      );

      expect(identical(patched, original), isFalse);
      expect(patched?['id'], 'snapshot-new');
      expect(patched?['score'], 88);

      final stale = patchRoomSnapshotForRealtimeUpdate(
        snapshot: patched,
        roomId: 'room-a',
        update: const RealtimeHomeUpdate(
          kind: RealtimeHomeUpdateKind.roomSnapshotUpdated,
          eventType: 'room.snapshot.updated',
          roomId: 'room-a',
          snapshotId: 'snapshot-stale',
          roomSnapshot: {
            'id': 'snapshot-stale',
            'roomId': 'room-a',
            'score': 10,
            'metrics': {'deductions': []},
            'formulaVersion': 'mousekeeper-cleanliness-v1',
            'calculatedAt': '2026-07-13T08:00:00.000Z',
          },
        ),
      );

      expect(identical(stale, patched), isTrue);
    },
  );

  test('room content reducer patches only changed slices', () {
    final current = RoomContent(
      commands: const [
        {'id': 'command-a', 'status': 'DELIVERED'},
      ],
      proposals: const [
        {'id': 'proposal-a', 'status': 'OPEN'},
      ],
      executions: const [
        {
          'execution': {'id': 'execution-a', 'status': 'EXECUTING'},
          'proposal': null,
        },
      ],
      activity: const [
        {'eventType': 'proposal.created'},
      ],
      snapshot: const {
        'id': 'snapshot-a',
        'roomId': 'room-a',
        'score': 50,
        'calculatedAt': '2026-07-13T09:00:00.000Z',
      },
      isOffline: false,
    );

    final patched = reduceRoomContentForRealtimeUpdate(
      current: current,
      roomId: 'room-a',
      update: const RealtimeHomeUpdate(
        kind: RealtimeHomeUpdateKind.executionStatus,
        eventType: 'execution.updated',
        roomId: 'room-a',
        executionId: 'execution-a',
        executionStatus: 'SUCCEEDED',
      ),
    );

    expect(identical(patched, current), isFalse);
    expect(identical(patched.commands, current.commands), isTrue);
    expect(identical(patched.proposals, current.proposals), isTrue);
    expect(identical(patched.activity, current.activity), isTrue);
    expect(
      (patched.executions.single['execution'] as Map)['status'],
      'SUCCEEDED',
    );
  });

  test('room content reducer returns same object for unrelated events', () {
    final current = RoomContent(
      commands: const [],
      proposals: const [],
      executions: const [],
      activity: const [],
      snapshot: null,
      isOffline: false,
    );

    final patched = reduceRoomContentForRealtimeUpdate(
      current: current,
      roomId: 'room-a',
      update: const RealtimeHomeUpdate(
        kind: RealtimeHomeUpdateKind.executionStatus,
        eventType: 'execution.updated',
        roomId: 'room-b',
        executionId: 'execution-a',
        executionStatus: 'SUCCEEDED',
      ),
    );

    expect(identical(patched, current), isTrue);
  });
}
