import 'package:drift/native.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/core/sync/realtime_controller.dart';
import 'package:mousekeeper/core/sync/mutation_queue.dart';
import 'package:mousekeeper/features/auth/connection_gate_controller.dart';
import 'package:mousekeeper/features/home/home_controller.dart';
import 'package:mousekeeper/storage/app_database.dart';
import 'package:mousekeeper/storage/display_cache.dart';

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

  test('paired device event adds one device without summary invalidation', () {
    final current = _homeData();
    final patched = reduceHomeDataForRealtimeUpdate(
      current: current,
      update: const RealtimeHomeUpdate(
        kind: RealtimeHomeUpdateKind.devicePaired,
        eventType: 'device.paired',
        deviceId: 'device-c',
        device: {
          'id': 'device-c',
          'platform': 'WINDOWS',
          'deviceName': 'New desktop',
          'status': 'ACTIVE',
          'lastSeenAt': null,
          'createdAt': '2026-07-13T01:02:03.000Z',
        },
      ),
      activeDeviceIds: const {'device-a', 'device-b', 'device-c'},
      activeRoomIds: const {'room-a'},
    )!;

    expect(patched.devices.map((item) => item['id']), [
      'device-a',
      'device-b',
      'device-c',
    ]);
    expect(patched.devices.last['presence'], 'OFFLINE');
    expect(patched.rooms, same(current.rooms));
  });

  test('paired device event outside the active gate is ignored', () {
    final current = _homeData();
    final patched = reduceHomeDataForRealtimeUpdate(
      current: current,
      update: const RealtimeHomeUpdate(
        kind: RealtimeHomeUpdateKind.devicePaired,
        eventType: 'device.paired',
        deviceId: 'device-c',
        device: {'id': 'device-c', 'status': 'ACTIVE'},
      ),
      activeDeviceIds: const {'device-a', 'device-b'},
      activeRoomIds: const {'room-a'},
    );

    expect(patched, same(current));
  });

  test('created room event appends the newly authoritative room', () {
    final current = _homeData();
    final patched = reduceHomeDataForRealtimeUpdate(
      current: current,
      update: const RealtimeHomeUpdate(
        kind: RealtimeHomeUpdateKind.roomCreated,
        eventType: 'room.created',
        roomId: 'room-b',
        room: {
          'id': 'room-b',
          'desktopDeviceId': 'device-a',
          'name': 'Reports',
          'rootAlias': 'reports',
          'status': 'ACTIVE',
          'createdAt': '2026-07-15T01:02:03.000Z',
        },
      ),
      activeDeviceIds: const {'device-a', 'device-b'},
      activeRoomIds: const {'room-a', 'room-b'},
    )!;

    expect(patched.rooms.map((room) => room['id']), ['room-a', 'room-b']);
    expect(patched.rooms.last['name'], 'Reports');
    expect(patched.rooms.last['pendingProposalCount'], 0);
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

  test(
    'connection safety reconcile does not reload the home summary projection',
    () async {
      final database = AppDatabase.forTesting(NativeDatabase.memory());
      final cache = DisplayCache(database, 'user-a');
      final api = _FakeConnectionControlApi()
        ..devices = [
          {
            'id': 'device-a',
            'deviceName': 'Desk',
            'platform': 'WINDOWS',
            'status': 'ACTIVE',
          },
        ]
        ..rooms = [
          {
            'id': 'room-a',
            'desktopDeviceId': 'device-a',
            'name': 'Downloads',
            'rootAlias': 'root:downloads',
            'status': 'ACTIVE',
          },
        ];
      var summaryCalls = 0;
      final container = ProviderContainer(
        overrides: [
          connectionControlApiProvider.overrideWithValue(api),
          displayCacheProvider.overrideWithValue(cache),
          mutationQueueProvider.overrideWithValue(
            MutationQueue(
              database,
              (
                _,
                _, {
                String? idempotencyKey,
                String? expectedOwnerUid,
              }) async => <String, dynamic>{},
              () => 'user-a',
            ),
          ),
          homeSummaryFetcherProvider.overrideWithValue(() async {
            summaryCalls++;
            return {
              'devices': [
                {
                  'id': 'device-a',
                  'status': 'ACTIVE',
                  'presence': 'ONLINE_IDLE',
                },
              ],
              'rooms': [
                {
                  'id': 'room-a',
                  'desktopDeviceId': 'device-a',
                  'status': 'ACTIVE',
                  'pendingProposalCount': 0,
                  'latestExecutionStatus': null,
                  'cleanlinessScore': null,
                  'cleanlinessFormulaVersion': null,
                  'cleanlinessCalculatedAt': null,
                },
              ],
              'character': null,
            };
          }),
        ],
      );
      final gateSubscription = container.listen(
        connectionGateControllerProvider,
        (_, _) {},
        fireImmediately: true,
      );
      ProviderSubscription<AsyncValue<HomeData>>? homeSubscription;
      addTearDown(() async {
        homeSubscription?.close();
        gateSubscription.close();
        container.dispose();
        await database.close();
      });

      await container.read(connectionGateControllerProvider.future);
      homeSubscription = container.listen(
        homeControllerProvider,
        (_, _) {},
        fireImmediately: true,
      );
      await container.read(homeControllerProvider.future);
      expect(summaryCalls, 1);

      api.devices = [
        {
          'id': 'device-a',
          'deviceName': 'Renamed desk',
          'platform': 'WINDOWS',
          'status': 'ACTIVE',
        },
      ];
      expect(
        await container
            .read(connectionGateControllerProvider.notifier)
            .reconcile(),
        isTrue,
      );
      await Future<void>.delayed(Duration.zero);

      expect(summaryCalls, 1);
      expect(
        container
            .read(connectionGateControllerProvider)
            .requireValue
            .devices
            .single['deviceName'],
        'Renamed desk',
      );
    },
  );
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

class _FakeConnectionControlApi implements ConnectionControlApi {
  List<Map<String, dynamic>> devices = [];
  List<Map<String, dynamic>> rooms = [];

  @override
  Future<ConnectionGateData> summary() async => ConnectionGateData(
    devices: devices.map(Map<String, dynamic>.from).toList(),
    rooms: rooms.map(Map<String, dynamic>.from).toList(),
  );

  @override
  Future<void> claimPairing(String code) async {}

  @override
  Future<void> revokeDevice(String deviceId, String idempotencyKey) async {}

  @override
  Future<void> removeRoom(String roomId, String idempotencyKey) async {}
}
