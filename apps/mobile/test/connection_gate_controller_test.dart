import 'dart:async';

import 'package:dio/dio.dart';
import 'package:drift/native.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/features/auth/connection_gate_controller.dart';
import 'package:mousekeeper/core/sync/connection_lifecycle.dart';
import 'package:mousekeeper/storage/app_database.dart';
import 'package:mousekeeper/storage/display_cache.dart';

void main() {
  late AppDatabase database;
  late _ControllableDisplayCache cache;
  late _FakeConnectionControlApi api;
  late ProviderContainer container;
  ProviderSubscription<AsyncValue<ConnectionGateData>>? subscription;

  setUp(() {
    database = AppDatabase.forTesting(NativeDatabase.memory());
    cache = _ControllableDisplayCache(database, 'user-a');
    api = _FakeConnectionControlApi();
    container = ProviderContainer(
      overrides: [
        connectionControlApiProvider.overrideWithValue(api),
        displayCacheProvider.overrideWithValue(cache),
        connectionDelayProvider.overrideWithValue((_) async {}),
        connectionMutationTimeoutProvider.overrideWithValue(
          const Duration(milliseconds: 50),
        ),
      ],
    );
  });

  void startGate() {
    subscription = container.listen(
      connectionGateControllerProvider,
      (_, _) {},
      fireImmediately: true,
    );
  }

  tearDown(() async {
    subscription?.close();
    container.dispose();
    await database.close();
  });

  test('stale cached device cannot open the main gate', () async {
    await cache.replaceDevices([
      {'id': 'stale-device', 'status': 'ACTIVE'},
    ]);
    await cache.replaceRooms([
      {'id': 'stale-room', 'desktopDeviceId': 'stale-device'},
    ]);
    startGate();

    final gate = await container.read(connectionGateControllerProvider.future);

    expect(gate.hasActiveDevice, isFalse);
    expect(await cache.devices(), isEmpty);
    expect(await cache.rooms(), isEmpty);
  });

  test(
    'a revoke during initial loading cannot open a stale main gate',
    () async {
      final staleDevices = Completer<List<Map<String, dynamic>>>();
      final staleRooms = Completer<List<Map<String, dynamic>>>();
      api.deviceListCompleter = staleDevices;
      api.roomListCompleter = staleRooms;
      startGate();
      await Future<void>.delayed(Duration.zero);

      container
          .read(connectionLifecycleEventProvider.notifier)
          .emit(
            const ConnectionLifecycleEvent(
              eventId: 'revoke-during-load',
              eventType: 'device.revoked',
              deviceId: 'device-a',
            ),
          );
      staleDevices.complete([
        {'id': 'device-a', 'status': 'ACTIVE'},
      ]);
      staleRooms.complete([
        {'id': 'room-a', 'desktopDeviceId': 'device-a', 'status': 'ACTIVE'},
      ]);

      final gate = await container.read(
        connectionGateControllerProvider.future,
      );
      expect(gate.hasActiveDevice, isFalse);
      expect(await cache.devices(), isEmpty);
      expect(await cache.rooms(), isEmpty);
    },
  );

  test(
    'only ACTIVE devices and rooms with an active device enter the gate',
    () async {
      api.devices = [
        {'id': 'active', 'status': 'ACTIVE'},
        {'id': 'legacy'},
        {'id': 'revoked', 'status': 'REVOKED'},
      ];
      api.rooms = [
        {'id': 'room-active', 'desktopDeviceId': 'active', 'status': 'ACTIVE'},
        {
          'id': 'room-revoked',
          'desktopDeviceId': 'revoked',
          'status': 'ACTIVE',
        },
        {'id': 'room-unbound', 'status': 'ACTIVE'},
      ];
      startGate();

      final gate = await container.read(
        connectionGateControllerProvider.future,
      );

      expect(gate.devices.map((item) => item['id']), ['active']);
      expect(gate.rooms.map((item) => item['id']), ['room-active']);
    },
  );

  test(
    'connection summary parser validates the lightweight gate aggregate',
    () {
      final summary = {
        'devices': [
          {'id': 'device-a', 'status': 'ACTIVE'},
        ],
        'rooms': [
          {'id': 'room-a', 'desktopDeviceId': 'device-a', 'status': 'ACTIVE'},
        ],
      };

      expect(connectionGateSummaryPath, '/v1/connections/summary');
      expect(
        connectionItemsFromSummary(summary, 'devices').single['id'],
        'device-a',
      );
      expect(
        connectionItemsFromSummary(summary, 'rooms').single['id'],
        'room-a',
      );
      expect(
        () => connectionItemsFromSummary({'devices': 'bad'}, 'devices'),
        throwsFormatException,
      );
    },
  );

  test(
    'reconcile reports and caches only an actual connection change',
    () async {
      api.devices = [
        {
          'id': 'device-a',
          'deviceName': 'Desk',
          'platform': 'WINDOWS',
          'status': 'ACTIVE',
          'lastSeenAt': '2026-07-13T10:00:00.000Z',
        },
      ];
      startGate();
      await container.read(connectionGateControllerProvider.future);
      final controller = container.read(
        connectionGateControllerProvider.notifier,
      );

      expect(await controller.reconcile(), isFalse);
      expect(cache.connectionReplacementCount, 1);

      api.devices = [
        {
          'id': 'device-a',
          'deviceName': 'Desk',
          'platform': 'WINDOWS',
          'status': 'ACTIVE',
          'lastSeenAt': '2026-07-13T10:00:05.000Z',
          'presence': 'ONLINE_IDLE',
        },
      ];
      expect(await controller.reconcile(), isFalse);
      expect(cache.connectionReplacementCount, 1);

      api.devices = [
        {
          'id': 'device-a',
          'deviceName': 'Renamed desk',
          'platform': 'WINDOWS',
          'status': 'ACTIVE',
          'lastSeenAt': '2026-07-13T10:00:10.000Z',
        },
      ];
      expect(await controller.reconcile(), isTrue);
      expect(cache.connectionReplacementCount, 2);
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

  test(
    'pairing claim creates main state only after authoritative confirmation',
    () async {
      startGate();
      await container.read(connectionGateControllerProvider.future);
      api.onClaim = (code) async {
        expect(code, '123456');
        api.devices = [
          {'id': 'paired-device', 'status': 'ACTIVE'},
        ];
      };

      await container
          .read(connectionGateControllerProvider.notifier)
          .claimAndConfirm('123456');

      expect(
        container
            .read(connectionGateControllerProvider)
            .requireValue
            .hasActiveDevice,
        isTrue,
      );
    },
  );

  test(
    'paired device event upserts the gate without a REST reconcile',
    () async {
      startGate();
      await container.read(connectionGateControllerProvider.future);
      final initialDeviceListCalls = api.deviceListCalls;
      final lifecycle = container.read(
        connectionLifecycleEventProvider.notifier,
      );

      lifecycle.emit(
        const ConnectionLifecycleEvent(
          eventId: 'paired-device-event',
          eventType: 'device.paired',
          deviceId: 'device-a',
          device: {
            'id': 'device-a',
            'platform': 'WINDOWS',
            'deviceName': 'Desktop',
            'status': 'ACTIVE',
            'lastSeenAt': null,
            'createdAt': '2026-07-13T01:02:03.000Z',
          },
        ),
      );

      for (var attempt = 0; attempt < 20; attempt++) {
        if (container
            .read(connectionGateControllerProvider)
            .requireValue
            .devices
            .isNotEmpty) {
          break;
        }
        await Future<void>.delayed(const Duration(milliseconds: 5));
      }

      final gate = container
          .read(connectionGateControllerProvider)
          .requireValue;
      expect(gate.devices.single['deviceName'], 'Desktop');
      expect(api.deviceListCalls, initialDeviceListCalls);
      expect((await cache.devices()).single['deviceName'], 'Desktop');
    },
  );

  test('incomplete paired device event falls back to one reconcile', () async {
    startGate();
    await container.read(connectionGateControllerProvider.future);
    final initialDeviceListCalls = api.deviceListCalls;
    api.devices = [
      {'id': 'device-a', 'status': 'ACTIVE', 'deviceName': 'Desktop'},
    ];
    container
        .read(connectionLifecycleEventProvider.notifier)
        .emit(
          const ConnectionLifecycleEvent(
            eventId: 'legacy-paired-device-event',
            eventType: 'device.paired',
            deviceId: 'device-a',
          ),
        );

    for (var attempt = 0; attempt < 20; attempt++) {
      if (container
          .read(connectionGateControllerProvider)
          .requireValue
          .devices
          .isNotEmpty) {
        break;
      }
      await Future<void>.delayed(const Duration(milliseconds: 5));
    }

    expect(api.deviceListCalls, initialDeviceListCalls + 1);
    expect(
      container
          .read(connectionGateControllerProvider)
          .requireValue
          .devices
          .single['id'],
      'device-a',
    );
  });

  test(
    'duplicate device disconnect is suppressed while request is pending',
    () async {
      api.devices = [
        {'id': 'device-a', 'status': 'ACTIVE'},
      ];
      api.rooms = [
        {'id': 'room-a', 'desktopDeviceId': 'device-a', 'status': 'ACTIVE'},
      ];
      startGate();
      await container.read(connectionGateControllerProvider.future);
      final pending = Completer<void>();
      api.revokeCompleter = pending;
      final controller = container.read(
        connectionGateControllerProvider.notifier,
      );

      final first = controller.disconnectDevice('device-a');
      await Future<void>.delayed(Duration.zero);
      final duplicate = controller.disconnectDevice('device-a');

      expect(api.revokeKeys, hasLength(1));
      pending.complete();
      await Future.wait([first, duplicate]);
      expect(
        container.read(connectionGateControllerProvider).requireValue.devices,
        isEmpty,
      );
      expect(await cache.rooms(), isEmpty);
    },
  );

  test('failed retry reuses the original idempotency key', () async {
    api.devices = [
      {'id': 'device-a', 'status': 'ACTIVE'},
    ];
    startGate();
    await container.read(connectionGateControllerProvider.future);
    api.revokeError = DioException(
      requestOptions: RequestOptions(path: '/v1/devices/device-a'),
      type: DioExceptionType.connectionError,
    );
    final controller = container.read(
      connectionGateControllerProvider.notifier,
    );

    await controller.disconnectDevice('device-a');
    final failed = container
        .read(connectionGateControllerProvider)
        .requireValue
        .operation(DisconnectKind.device, 'device-a');
    expect(failed?.phase, DisconnectPhase.failed);

    api.revokeError = null;
    await controller.retryDisconnect(DisconnectKind.device, 'device-a');

    expect(api.revokeKeys, hasLength(2));
    expect(api.revokeKeys[1], api.revokeKeys[0]);
    expect(
      container.read(connectionGateControllerProvider).requireValue.devices,
      isEmpty,
    );
  });

  test('room disconnect purges room cache but keeps device pairing', () async {
    api.devices = [
      {'id': 'device-a', 'status': 'ACTIVE'},
    ];
    api.rooms = [
      {'id': 'room-a', 'desktopDeviceId': 'device-a', 'status': 'ACTIVE'},
    ];
    startGate();
    await container.read(connectionGateControllerProvider.future);
    await cache.replaceCommands('room-a', [
      {'id': 'command-a'},
    ]);

    await container
        .read(connectionGateControllerProvider.notifier)
        .disconnectRoom('room-a');

    final gate = container.read(connectionGateControllerProvider).requireValue;
    expect(gate.devices, hasLength(1));
    expect(gate.rooms, isEmpty);
    expect(await cache.commands('room-a'), isEmpty);
    expect(api.roomKeys.single, isNotEmpty);
  });

  test('a timed out disconnect becomes failed instead of hanging', () async {
    api.devices = [
      {'id': 'device-a', 'status': 'ACTIVE'},
    ];
    startGate();
    await container.read(connectionGateControllerProvider.future);
    api.revokeCompleter = Completer<void>();

    await container
        .read(connectionGateControllerProvider.notifier)
        .disconnectDevice('device-a');

    final operation = container
        .read(connectionGateControllerProvider)
        .requireValue
        .operation(DisconnectKind.device, 'device-a');
    expect(operation?.phase, DisconnectPhase.failed);
  });

  test(
    'late reconcile cannot resurrect a successfully revoked device',
    () async {
      api.devices = [
        {'id': 'device-a', 'status': 'ACTIVE'},
      ];
      startGate();
      await container.read(connectionGateControllerProvider.future);
      final staleDevices = Completer<List<Map<String, dynamic>>>();
      final staleRooms = Completer<List<Map<String, dynamic>>>();
      api.deviceListCompleter = staleDevices;
      api.roomListCompleter = staleRooms;
      final controller = container.read(
        connectionGateControllerProvider.notifier,
      );

      final reconciling = controller.reconcile();
      await Future<void>.delayed(Duration.zero);
      await controller.disconnectDevice('device-a');
      staleDevices.complete([
        {'id': 'device-a', 'status': 'ACTIVE'},
      ]);
      staleRooms.complete(const []);
      await reconciling;

      expect(
        container.read(connectionGateControllerProvider).requireValue.devices,
        isEmpty,
      );
    },
  );

  test('revoke purge follows an in-flight stale cache replacement', () async {
    api.devices = [
      {'id': 'device-a', 'status': 'ACTIVE'},
    ];
    api.rooms = [
      {'id': 'room-a', 'desktopDeviceId': 'device-a', 'status': 'ACTIVE'},
    ];
    startGate();
    await container.read(connectionGateControllerProvider.future);
    final controller = container.read(
      connectionGateControllerProvider.notifier,
    );
    api.devices = [
      {'id': 'device-a', 'deviceName': 'Updated', 'status': 'ACTIVE'},
    ];
    cache.blockNextConnectionReplacement();

    final reconciling = controller.reconcile();
    await cache.connectionReplacementStarted;
    final disconnecting = controller.disconnectDevice('device-a');
    await Future<void>.delayed(Duration.zero);
    cache.releaseConnectionReplacement();
    await Future.wait([reconciling, disconnecting]);

    expect(
      container.read(connectionGateControllerProvider).requireValue.devices,
      isEmpty,
    );
    expect(await cache.devices(), isEmpty);
    expect(await cache.rooms(), isEmpty);
  });

  test('consecutive room removal events cannot resurrect each other', () async {
    api.devices = [
      {'id': 'device-a', 'status': 'ACTIVE'},
    ];
    api.rooms = [
      {'id': 'room-a', 'desktopDeviceId': 'device-a', 'status': 'ACTIVE'},
      {'id': 'room-b', 'desktopDeviceId': 'device-a', 'status': 'ACTIVE'},
    ];
    startGate();
    await container.read(connectionGateControllerProvider.future);

    final lifecycle = container.read(connectionLifecycleEventProvider.notifier);
    lifecycle.emit(
      const ConnectionLifecycleEvent(
        eventId: 'event-a',
        eventType: 'room.removed',
        roomId: 'room-a',
      ),
    );
    lifecycle.emit(
      const ConnectionLifecycleEvent(
        eventId: 'event-b',
        eventType: 'room.removed',
        roomId: 'room-b',
      ),
    );
    for (var attempt = 0; attempt < 20; attempt++) {
      if (container
          .read(connectionGateControllerProvider)
          .requireValue
          .rooms
          .isEmpty) {
        break;
      }
      await Future<void>.delayed(const Duration(milliseconds: 5));
    }

    expect(
      container.read(connectionGateControllerProvider).requireValue.rooms,
      isEmpty,
    );
  });
}

class _ControllableDisplayCache extends DisplayCache {
  _ControllableDisplayCache(super.database, super.ownerUid);

  Completer<void>? _replacementStarted;
  Completer<void>? _replacementRelease;
  int connectionReplacementCount = 0;

  Future<void> get connectionReplacementStarted =>
      _replacementStarted?.future ?? Future<void>.value();

  void blockNextConnectionReplacement() {
    _replacementStarted = Completer<void>();
    _replacementRelease = Completer<void>();
  }

  void releaseConnectionReplacement() => _replacementRelease?.complete();

  @override
  Future<void> replaceConnectionState({
    required List<Map<String, dynamic>> devices,
    required List<Map<String, dynamic>> rooms,
  }) async {
    connectionReplacementCount++;
    final started = _replacementStarted;
    final release = _replacementRelease;
    if (started != null && release != null) {
      started.complete();
      await release.future;
      if (identical(_replacementStarted, started)) {
        _replacementStarted = null;
        _replacementRelease = null;
      }
    }
    await super.replaceConnectionState(devices: devices, rooms: rooms);
  }
}

class _FakeConnectionControlApi implements ConnectionControlApi {
  List<Map<String, dynamic>> devices = [];
  List<Map<String, dynamic>> rooms = [];
  int deviceListCalls = 0;
  int roomListCalls = 0;
  Future<void> Function(String code)? onClaim;
  Completer<void>? revokeCompleter;
  Completer<List<Map<String, dynamic>>>? deviceListCompleter;
  Completer<List<Map<String, dynamic>>>? roomListCompleter;
  Object? revokeError;
  final List<String> revokeKeys = [];
  final List<String> roomKeys = [];

  @override
  Future<ConnectionGateData> summary() async {
    final results = await Future.wait([_listDevices(), _listRooms()]);
    return ConnectionGateData(devices: results[0], rooms: results[1]);
  }

  Future<List<Map<String, dynamic>>> _listDevices() async {
    deviceListCalls++;
    final completer = deviceListCompleter;
    if (completer != null) {
      deviceListCompleter = null;
      return completer.future;
    }
    return devices.map(Map<String, dynamic>.from).toList();
  }

  Future<List<Map<String, dynamic>>> _listRooms() async {
    roomListCalls++;
    final completer = roomListCompleter;
    if (completer != null) {
      roomListCompleter = null;
      return completer.future;
    }
    return rooms.map(Map<String, dynamic>.from).toList();
  }

  @override
  Future<void> claimPairing(String code) async {
    await onClaim?.call(code);
  }

  @override
  Future<void> revokeDevice(String deviceId, String idempotencyKey) async {
    revokeKeys.add(idempotencyKey);
    final error = revokeError;
    if (error != null) throw error;
    await revokeCompleter?.future;
    devices = devices.where((item) => item['id'] != deviceId).toList();
    rooms = rooms.where((item) => item['desktopDeviceId'] != deviceId).toList();
  }

  @override
  Future<void> removeRoom(String roomId, String idempotencyKey) async {
    roomKeys.add(idempotencyKey);
    rooms = rooms.where((item) => item['id'] != roomId).toList();
  }
}
