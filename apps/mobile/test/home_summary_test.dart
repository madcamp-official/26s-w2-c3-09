import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/core/sync/realtime_controller.dart';
import 'package:mousekeeper/features/home/home_controller.dart';

void main() {
  test('home projection is parsed from exactly one summary fetch', () async {
    var calls = 0;
    final summary = await fetchHomeSummary(() async {
      calls++;
      return {
        'devices': [
          {'id': 'device-a', 'status': 'ACTIVE', 'presence': 'ONLINE_IDLE'},
        ],
        'rooms': [
          {
            'id': 'room-a',
            'status': 'ACTIVE',
            'pendingProposalCount': 2,
            'latestExecutionStatus': 'SUCCEEDED',
            'cleanlinessScore': 88,
            'cleanlinessFormulaVersion': 'mousekeeper-cleanliness-v1',
            'cleanlinessCalculatedAt': '2026-07-13T10:00:00.000Z',
          },
        ],
        'character': {'affinityTotal': 7},
      };
    });

    expect(calls, 1);
    expect(summary.devices.single['presence'], 'ONLINE_IDLE');
    expect(summary.rooms.single['pendingProposalCount'], 2);
    expect(summary.character?['affinityTotal'], 7);
    expect(homeSummaryPath, '/v1/home/summary');
  });

  test('invalid summary shape fails instead of inventing home data', () {
    expect(
      () => HomeSummaryPayload.fromJson({
        'devices': const <Object>[],
        'rooms': 'not-a-list',
        'character': null,
      }),
      throwsFormatException,
    );
  });

  test('presence update patches only its active device', () {
    final current = _homeData();
    final patched = reduceHomeDataForRealtimeUpdate(
      current: current,
      update: const RealtimeHomeUpdate(
        kind: RealtimeHomeUpdateKind.presence,
        eventType: 'presence.updated',
        deviceId: 'device-a',
        presence: 'ONLINE_EXECUTING',
      ),
      activeDeviceIds: const {'device-a', 'device-b'},
      activeRoomIds: const {'room-a'},
    )!;

    expect(patched.devices[0]['presence'], 'ONLINE_EXECUTING');
    expect(patched.devices[1]['presence'], 'OFFLINE');
    expect(patched.rooms, same(current.rooms));
  });

  test(
    'targeted lifecycle and execution updates avoid summary invalidation',
    () {
      final current = _homeData();
      final execution = reduceHomeDataForRealtimeUpdate(
        current: current,
        update: const RealtimeHomeUpdate(
          kind: RealtimeHomeUpdateKind.executionStatus,
          eventType: 'execution.updated',
          roomId: 'room-a',
          executionStatus: 'FAILED',
        ),
        activeDeviceIds: const {'device-a', 'device-b'},
        activeRoomIds: const {'room-a'},
      )!;
      expect(execution.rooms.single['latestExecutionStatus'], 'FAILED');

      final proposal = reduceHomeDataForRealtimeUpdate(
        current: execution,
        update: const RealtimeHomeUpdate(
          kind: RealtimeHomeUpdateKind.proposalCreated,
          eventType: 'proposal.created',
          roomId: 'room-a',
          proposalId: 'proposal-a',
          proposalStatus: 'OPEN',
          pendingProposalCount: 4,
        ),
        activeDeviceIds: const {'device-a', 'device-b'},
        activeRoomIds: const {'room-a'},
      )!;
      expect(proposal.rooms.single['pendingProposalCount'], 4);

      final removed = reduceHomeDataForRealtimeUpdate(
        current: proposal,
        update: const RealtimeHomeUpdate(
          kind: RealtimeHomeUpdateKind.decisionCreated,
          eventType: 'decision.created',
          roomId: 'room-a',
          proposalId: 'proposal-a',
          decisionId: 'decision-a',
          decisionType: 'APPROVE',
          proposalStatus: 'APPROVED',
          commandStatus: 'APPROVED',
          pendingProposalCount: 0,
        ),
        activeDeviceIds: const {'device-a', 'device-b'},
        activeRoomIds: const {'room-a'},
      )!;
      expect(removed.rooms.single['pendingProposalCount'], 0);

      final snapshot = reduceHomeDataForRealtimeUpdate(
        current: removed,
        update: const RealtimeHomeUpdate(
          kind: RealtimeHomeUpdateKind.roomSnapshotUpdated,
          eventType: 'room.snapshot.updated',
          roomId: 'room-a',
          snapshotId: 'snapshot-a',
          roomSnapshot: {
            'id': 'snapshot-a',
            'roomId': 'room-a',
            'score': 88,
            'metrics': {
              'totalFileCount': 10,
              'managedFileCount': 8,
              'unorganizedFileCount': 2,
              'deductions': [],
            },
            'formulaVersion': 'mousekeeper-cleanliness-v1',
            'calculatedAt': '2026-07-13T10:00:00.000Z',
          },
        ),
        activeDeviceIds: const {'device-a', 'device-b'},
        activeRoomIds: const {'room-a'},
      )!;
      expect(snapshot.rooms.single['cleanlinessScore'], 88);
      expect(
        snapshot.rooms.single['cleanlinessFormulaVersion'],
        'mousekeeper-cleanliness-v1',
      );

      final revoked = reduceHomeDataForRealtimeUpdate(
        current: snapshot,
        update: const RealtimeHomeUpdate(
          kind: RealtimeHomeUpdateKind.deviceRemoved,
          eventType: 'device.revoked',
          deviceId: 'device-a',
        ),
        activeDeviceIds: const {'device-b'},
        activeRoomIds: const <String>{},
      )!;
      expect(revoked.devices.map((item) => item['id']), ['device-b']);
      expect(revoked.rooms, isEmpty);
    },
  );

  test('events without a complete projection request one summary fallback', () {
    expect(
      reduceHomeDataForRealtimeUpdate(
        current: _homeData(),
        update: const RealtimeHomeUpdate(
          kind: RealtimeHomeUpdateKind.refreshSummary,
          eventType: 'room.snapshot.updated',
        ),
        activeDeviceIds: const {'device-a', 'device-b'},
        activeRoomIds: const {'room-a'},
      ),
      isNull,
    );
    expect(
      reduceHomeDataForRealtimeUpdate(
        current: _homeData(),
        update: const RealtimeHomeUpdate(
          kind: RealtimeHomeUpdateKind.proposalCreated,
          eventType: 'proposal.created',
          roomId: 'room-a',
          proposalId: 'proposal-a',
          proposalStatus: 'OPEN',
        ),
        activeDeviceIds: const {'device-a', 'device-b'},
        activeRoomIds: const {'room-a'},
      ),
      isNull,
    );
  });
}

HomeData _homeData() => const HomeData(
  devices: [
    {'id': 'device-a', 'presence': 'ONLINE_IDLE'},
    {'id': 'device-b', 'presence': 'OFFLINE'},
  ],
  rooms: [
    {
      'id': 'room-a',
      'desktopDeviceId': 'device-a',
      'pendingProposalCount': 1,
      'latestExecutionStatus': 'SUCCEEDED',
      'cleanlinessScore': 52,
      'cleanlinessFormulaVersion': 'mousekeeper-cleanliness-v1',
      'cleanlinessCalculatedAt': '2026-07-13T09:00:00.000Z',
    },
  ],
  isOffline: false,
  outboxPending: 0,
  outboxFailed: 0,
);
