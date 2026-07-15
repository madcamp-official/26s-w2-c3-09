import 'dart:async';

import 'package:dio/dio.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';

import '../../core/network/api_client.dart';
import '../../core/sync/connection_lifecycle.dart';
import '../../storage/display_cache.dart';

enum DisconnectKind { device, room }

enum DisconnectPhase { disconnecting, failed }

class DisconnectOperation {
  const DisconnectOperation({
    required this.kind,
    required this.aggregateId,
    required this.idempotencyKey,
    required this.phase,
    this.message,
  });

  final DisconnectKind kind;
  final String aggregateId;
  final String idempotencyKey;
  final DisconnectPhase phase;
  final String? message;

  DisconnectOperation copyWith({DisconnectPhase? phase, String? message}) =>
      DisconnectOperation(
        kind: kind,
        aggregateId: aggregateId,
        idempotencyKey: idempotencyKey,
        phase: phase ?? this.phase,
        message: message,
      );
}

class ConnectionGateData {
  ConnectionGateData({
    required List<Map<String, dynamic>> devices,
    required List<Map<String, dynamic>> rooms,
    Map<String, DisconnectOperation> operations = const {},
  }) : devices = List.unmodifiable(devices),
       rooms = List.unmodifiable(rooms),
       operations = Map.unmodifiable(operations);

  final List<Map<String, dynamic>> devices;
  final List<Map<String, dynamic>> rooms;
  final Map<String, DisconnectOperation> operations;

  bool get hasActiveDevice => devices.isNotEmpty;

  DisconnectOperation? operation(DisconnectKind kind, String aggregateId) =>
      operations[_operationKey(kind, aggregateId)];

  ConnectionGateData copyWith({
    List<Map<String, dynamic>>? devices,
    List<Map<String, dynamic>>? rooms,
    Map<String, DisconnectOperation>? operations,
  }) => ConnectionGateData(
    devices: devices ?? this.devices,
    rooms: rooms ?? this.rooms,
    operations: operations ?? this.operations,
  );
}

bool isActiveConnectionItem(Map<String, dynamic> value) {
  final status = value['status'];
  return status is String && status.toUpperCase() == 'ACTIVE';
}

bool connectionItemsEquivalent(
  List<Map<String, dynamic>> left,
  List<Map<String, dynamic>> right, {
  Set<String>? stableKeys,
}) {
  if (left.length != right.length) return false;
  final rightById = <String, Map<String, dynamic>>{
    for (final item in right)
      if (item['id'] is String) item['id'] as String: item,
  };
  if (rightById.length != right.length) return false;
  for (final item in left) {
    final id = item['id'];
    final other = rightById[id];
    if (id is! String || other == null) return false;
    if (stableKeys == null) {
      if (!_jsonEquivalent(item, other)) return false;
      continue;
    }
    for (final key in stableKeys) {
      if (!_jsonEquivalent(item[key], other[key])) return false;
    }
  }
  return true;
}

const _stableDeviceConnectionKeys = <String>{
  'id',
  'status',
  'deviceName',
  'platform',
};

const _stableRoomConnectionKeys = <String>{
  'id',
  'status',
  'desktopDeviceId',
  'name',
  'rootAlias',
};

bool _jsonEquivalent(Object? left, Object? right) {
  if (identical(left, right)) return true;
  if (left is Map && right is Map) {
    if (left.length != right.length) return false;
    for (final entry in left.entries) {
      if (!right.containsKey(entry.key) ||
          !_jsonEquivalent(entry.value, right[entry.key])) {
        return false;
      }
    }
    return true;
  }
  if (left is List && right is List) {
    if (left.length != right.length) return false;
    for (var index = 0; index < left.length; index++) {
      if (!_jsonEquivalent(left[index], right[index])) return false;
    }
    return true;
  }
  return left == right;
}

String disconnectErrorMessage(Object error) {
  if (error is DioException) {
    final data = error.response?.data;
    if (data is Map && data['code'] == 'NOT_FOUND') {
      return '이미 해제되었거나 더 이상 접근할 수 없습니다. 상태를 다시 확인해 주세요.';
    }
  }
  if (_isAmbiguousDisconnectError(error)) {
    return '서버에서 연결 해제 결과를 확인하지 못했습니다. 다시 시도해 주세요.';
  }
  return '연결 해제에 실패했습니다. 상태를 확인한 뒤 다시 시도해 주세요.';
}

abstract interface class ConnectionControlApi {
  Future<ConnectionGateData> summary();
  Future<void> claimPairing(String code);
  Future<void> revokeDevice(String deviceId, String idempotencyKey);
  Future<void> removeRoom(String roomId, String idempotencyKey);
}

const connectionGateSummaryPath = '/v1/connections/summary';

List<Map<String, dynamic>> connectionItemsFromSummary(
  Map<String, dynamic> summary,
  String key,
) {
  final raw = summary[key];
  if (raw is! List) {
    throw FormatException('INVALID_CONNECTION_SUMMARY_${key.toUpperCase()}');
  }
  return raw
      .map((item) {
        if (item is! Map) {
          throw FormatException(
            'INVALID_CONNECTION_SUMMARY_${key.toUpperCase()}',
          );
        }
        return Map<String, dynamic>.from(item);
      })
      .toList(growable: false);
}

class ApiConnectionControl implements ConnectionControlApi {
  ApiConnectionControl(this._api);

  final ApiClient _api;
  Future<ConnectionGateData>? _connectionSummaryInFlight;

  @override
  Future<ConnectionGateData> summary() {
    final existing = _connectionSummaryInFlight;
    if (existing != null) return existing;
    late final Future<ConnectionGateData> request;
    request = _api
        .get(connectionGateSummaryPath)
        .then(
          (summary) => ConnectionGateData(
            devices: connectionItemsFromSummary(summary, 'devices'),
            rooms: connectionItemsFromSummary(summary, 'rooms'),
          ),
        )
        .whenComplete(() {
          if (identical(_connectionSummaryInFlight, request)) {
            _connectionSummaryInFlight = null;
          }
        });
    _connectionSummaryInFlight = request;
    return request;
  }

  @override
  Future<void> claimPairing(String code) async {
    await _api.post('/v1/pairing-sessions/claim', {'code': code});
  }

  @override
  Future<void> revokeDevice(String deviceId, String idempotencyKey) async {
    await _api.delete(
      '/v1/devices/$deviceId',
      idempotencyKey: idempotencyKey,
      requestTimeout: const Duration(seconds: 10),
    );
  }

  @override
  Future<void> removeRoom(String roomId, String idempotencyKey) async {
    await _api.delete(
      '/v1/rooms/$roomId',
      idempotencyKey: idempotencyKey,
      requestTimeout: const Duration(seconds: 10),
    );
  }
}

final connectionControlApiProvider = Provider<ConnectionControlApi>(
  (ref) => ApiConnectionControl(ref.watch(apiClientProvider)),
);

typedef ConnectionDelay = Future<void> Function(Duration duration);

final connectionDelayProvider = Provider<ConnectionDelay>(
  (_) => Future<void>.delayed,
);

final connectionMutationTimeoutProvider = Provider<Duration>(
  (_) => const Duration(seconds: 10),
);

final connectionGateControllerProvider =
    AsyncNotifierProvider.autoDispose<
      ConnectionGateController,
      ConnectionGateData
    >(ConnectionGateController.new);

class ConnectionGateController extends AsyncNotifier<ConnectionGateData> {
  int _stateRevision = 0;
  Future<void> _mutationTail = Future<void>.value();

  @override
  Future<ConnectionGateData> build() async {
    ref.listen(connectionLifecycleEventProvider, (previous, next) {
      if (next == null || previous?.eventId == next.eventId) return;
      unawaited(_reduceLifecycle(next));
    });
    return _loadAuthoritativeFailClosed();
  }

  Future<void> retryLoad() async {
    _stateRevision++;
    state = const AsyncLoading();
    state = await AsyncValue.guard(_loadAuthoritativeFailClosed);
  }

  Future<bool> reconcile() async {
    if (state.asData?.value == null) {
      await retryLoad();
      return state.asData?.value != null;
    }
    final revision = _stateRevision;
    try {
      final refreshed = await _readAuthoritative();
      if (revision != _stateRevision) return false;
      return await _serializeMutation(() async {
        if (revision != _stateRevision) return false;
        final current = state.asData?.value;
        if (current == null) return false;
        final retainedOperations = <String, DisconnectOperation>{};
        for (final entry in current.operations.entries) {
          final operation = entry.value;
          final stillPresent = operation.kind == DisconnectKind.device
              ? refreshed.devices.any(
                  (device) => device['id'] == operation.aggregateId,
                )
              : refreshed.rooms.any(
                  (room) => room['id'] == operation.aggregateId,
                );
          if (stillPresent) retainedOperations[entry.key] = operation;
        }
        final next = refreshed.copyWith(operations: retainedOperations);
        final changed =
            !connectionItemsEquivalent(
              current.devices,
              next.devices,
              stableKeys: _stableDeviceConnectionKeys,
            ) ||
            !connectionItemsEquivalent(
              current.rooms,
              next.rooms,
              stableKeys: _stableRoomConnectionKeys,
            ) ||
            current.operations.length != next.operations.length;
        if (!changed) return false;
        final applyingRevision = ++_stateRevision;
        await _replaceAuthoritativeCache(refreshed);
        if (applyingRevision != _stateRevision) return false;
        state = AsyncData(next);
        return true;
      });
    } catch (_) {
      // A background reconciliation must never replace a valid main screen
      // with stale cache or an unverified error state.
      return false;
    }
  }

  Future<void> claimAndConfirm(String code) async {
    if (state.asData?.value.hasActiveDevice ?? false) {
      throw StateError('DEVICE_ALREADY_PAIRED');
    }
    await ref.read(connectionControlApiProvider).claimPairing(code);
    Object? latestError;
    for (var attempt = 0; attempt < 10; attempt++) {
      final revision = _stateRevision;
      try {
        final refreshed = await _readAuthoritative();
        if (refreshed.hasActiveDevice) {
          final applied = await _serializeMutation(() async {
            if (revision != _stateRevision) return false;
            final applyingRevision = ++_stateRevision;
            await _replaceAuthoritativeCache(refreshed);
            if (applyingRevision != _stateRevision) return false;
            state = AsyncData(refreshed);
            return true;
          });
          if (applied) return;
        }
      } catch (error) {
        latestError = error;
      }
      if (attempt < 9) {
        await ref.read(connectionDelayProvider)(const Duration(seconds: 1));
      }
    }
    throw StateError(
      latestError == null
          ? 'PAIRING_CONFIRMATION_TIMEOUT'
          : 'PAIRING_CONFIRMATION_FAILED: $latestError',
    );
  }

  Future<void> disconnectDevice(String deviceId) {
    return _disconnect(DisconnectKind.device, deviceId);
  }

  Future<void> disconnectRoom(String roomId) {
    return _disconnect(DisconnectKind.room, roomId);
  }

  Future<void> retryDisconnect(DisconnectKind kind, String aggregateId) {
    return _disconnect(kind, aggregateId, retry: true);
  }

  Future<void> _disconnect(
    DisconnectKind kind,
    String aggregateId, {
    bool retry = false,
  }) async {
    final current = state.asData?.value;
    if (current == null) return;
    final existing = current.operation(kind, aggregateId);
    if (existing?.phase == DisconnectPhase.disconnecting) return;
    if (!retry && existing?.phase == DisconnectPhase.failed) return;
    final operation = DisconnectOperation(
      kind: kind,
      aggregateId: aggregateId,
      idempotencyKey: existing?.idempotencyKey ?? const Uuid().v4(),
      phase: DisconnectPhase.disconnecting,
    );
    _stateRevision++;
    _setOperation(operation);
    try {
      final api = ref.read(connectionControlApiProvider);
      final timeout = ref.read(connectionMutationTimeoutProvider);
      if (kind == DisconnectKind.device) {
        await api
            .revokeDevice(aggregateId, operation.idempotencyKey)
            .timeout(timeout);
        await _removeDevice(aggregateId);
      } else {
        await api
            .removeRoom(aggregateId, operation.idempotencyKey)
            .timeout(timeout);
        await _removeRoom(aggregateId);
      }
    } catch (error) {
      var confirmed = false;
      if (_isAmbiguousDisconnectError(error)) {
        confirmed = await _confirmAbsent(kind, aggregateId);
      }
      if (confirmed) return;
      final latest = state.asData?.value;
      if (latest == null) return;
      _setOperation(
        operation.copyWith(
          phase: DisconnectPhase.failed,
          message: disconnectErrorMessage(error),
        ),
      );
    }
  }

  Future<bool> _confirmAbsent(DisconnectKind kind, String aggregateId) async {
    final deadline = DateTime.now().add(const Duration(seconds: 10));
    for (var attempt = 0; attempt < 5; attempt++) {
      final remaining = deadline.difference(DateTime.now());
      if (remaining <= Duration.zero) return false;
      try {
        final requestTimeout = remaining < const Duration(seconds: 2)
            ? remaining
            : const Duration(seconds: 2);
        final refreshed = await _readAuthoritative().timeout(requestTimeout);
        final remains = kind == DisconnectKind.device
            ? refreshed.devices.any((item) => item['id'] == aggregateId)
            : refreshed.rooms.any((item) => item['id'] == aggregateId);
        if (!remains) {
          final revision = _stateRevision;
          final applied = await _serializeMutation(() async {
            if (revision != _stateRevision) {
              final current = state.asData?.value;
              return current != null &&
                  !(kind == DisconnectKind.device
                      ? current.devices.any((item) => item['id'] == aggregateId)
                      : current.rooms.any((item) => item['id'] == aggregateId));
            }
            final applyingRevision = ++_stateRevision;
            await _replaceAuthoritativeCache(refreshed);
            if (applyingRevision != _stateRevision) return false;
            state = AsyncData(refreshed);
            return true;
          });
          if (applied) return true;
        }
      } catch (_) {
        // Keep the explicit failed state unless an authoritative read proves
        // that the aggregate was removed.
      }
      if (attempt < 4) {
        final delayRemaining = deadline.difference(DateTime.now());
        if (delayRemaining <= Duration.zero) return false;
        await ref.read(connectionDelayProvider)(
          delayRemaining < const Duration(seconds: 2)
              ? delayRemaining
              : const Duration(seconds: 2),
        );
      }
    }
    return false;
  }

  Future<ConnectionGateData> _readAuthoritative() async {
    final api = ref.read(connectionControlApiProvider);
    final summary = await api.summary();
    final devices = summary.devices
        .where(isActiveConnectionItem)
        .map((value) => Map<String, dynamic>.from(value))
        .toList(growable: false);
    final deviceIds = devices
        .map((device) => device['id'])
        .whereType<String>()
        .toSet();
    final rooms = summary.rooms
        .where(isActiveConnectionItem)
        .where((room) {
          final desktopDeviceId = room['desktopDeviceId'];
          return desktopDeviceId is String &&
              deviceIds.contains(desktopDeviceId);
        })
        .map((value) => Map<String, dynamic>.from(value))
        .toList(growable: false);
    return ConnectionGateData(devices: devices, rooms: rooms);
  }

  Future<ConnectionGateData> _loadAuthoritativeFailClosed() async {
    while (true) {
      final revision = _stateRevision;
      final refreshed = await _readAuthoritative();
      final applied = await _serializeMutation(() async {
        if (revision != _stateRevision) return false;
        final applyingRevision = ++_stateRevision;
        await _replaceAuthoritativeCache(refreshed);
        return applyingRevision == _stateRevision;
      });
      if (applied) return refreshed;
    }
  }

  Future<void> _replaceAuthoritativeCache(ConnectionGateData data) async {
    final cache = ref.read(displayCacheProvider);
    final devices = data.devices;
    final rooms = data.rooms;
    if (devices.isEmpty) {
      await cache.purgeConnectionDisplayData();
    } else {
      await cache.replaceConnectionState(devices: devices, rooms: rooms);
    }
  }

  Future<void> _reduceLifecycle(ConnectionLifecycleEvent event) async {
    if (event.eventType == 'device.revoked' && event.deviceId != null) {
      await _removeDevice(event.deviceId!);
      return;
    }
    if (event.eventType == 'room.removed' && event.roomId != null) {
      await _removeRoom(event.roomId!);
      return;
    }
    if (event.eventType == 'device.paired') {
      final device = event.device;
      if (device != null) {
        await _upsertDevice(device);
      } else {
        await reconcile();
      }
      return;
    }
    if (event.eventType == 'room.created') {
      final room = event.room;
      if (room != null) {
        await _upsertRoom(room);
      } else {
        await reconcile();
      }
    }
  }

  Future<void> _upsertDevice(Map<String, dynamic> device) async {
    if (!isActiveConnectionItem(device)) return;
    final deviceId = device['id'];
    if (deviceId is! String) return;
    _stateRevision++;
    await _serializeMutation(() async {
      final current =
          state.asData?.value ??
          ConnectionGateData(devices: const [], rooms: const []);
      final sanitizedDevice = Map<String, dynamic>.from(device);
      final devices = <Map<String, dynamic>>[
        for (final item in current.devices)
          if (item['id'] != deviceId) item,
        sanitizedDevice,
      ];
      final nextOperations = Map<String, DisconnectOperation>.from(
        current.operations,
      )..remove(_operationKey(DisconnectKind.device, deviceId));
      final next = current.copyWith(
        devices: devices,
        operations: nextOperations,
      );
      await _replaceAuthoritativeCache(next);
      state = AsyncData(next);
    });
  }

  Future<void> _upsertRoom(Map<String, dynamic> room) async {
    if (!isActiveConnectionItem(room)) return;
    final roomId = room['id'];
    final desktopDeviceId = room['desktopDeviceId'];
    if (roomId is! String || desktopDeviceId is! String) return;
    _stateRevision++;
    await _serializeMutation(() async {
      final current = state.asData?.value;
      if (current == null ||
          !current.devices.any((device) => device['id'] == desktopDeviceId)) {
        return;
      }
      final sanitizedRoom = Map<String, dynamic>.from(room);
      final rooms = <Map<String, dynamic>>[
        for (final item in current.rooms)
          if (item['id'] != roomId) item,
        sanitizedRoom,
      ];
      final nextOperations = Map<String, DisconnectOperation>.from(
        current.operations,
      )..remove(_operationKey(DisconnectKind.room, roomId));
      final next = current.copyWith(rooms: rooms, operations: nextOperations);
      await _replaceAuthoritativeCache(next);
      state = AsyncData(next);
    });
  }

  Future<void> _removeDevice(String deviceId) async {
    _stateRevision++;
    await _serializeMutation(() async {
      await ref.read(displayCacheProvider).removeDeviceCascade(deviceId);
      var current = state.asData?.value;
      if (current == null) return;
      final remainingDevices = current.devices
          .where((device) => device['id'] != deviceId)
          .toList(growable: false);
      if (remainingDevices.isEmpty) {
        await ref.read(displayCacheProvider).purgeConnectionDisplayData();
        current = state.asData?.value;
        if (current == null) return;
      }
      final latestRemainingDevices = current.devices
          .where((device) => device['id'] != deviceId)
          .toList(growable: false);
      final latestRemovedRoomIds = current.rooms
          .where((room) => room['desktopDeviceId'] == deviceId)
          .map((room) => room['id'])
          .whereType<String>()
          .toSet();
      final latestRemainingRooms = current.rooms
          .where((room) => !latestRemovedRoomIds.contains(room['id']))
          .toList(growable: false);
      final latestOperations = Map<String, DisconnectOperation>.from(
        current.operations,
      )..remove(_operationKey(DisconnectKind.device, deviceId));
      for (final roomId in latestRemovedRoomIds) {
        latestOperations.remove(_operationKey(DisconnectKind.room, roomId));
      }
      state = AsyncData(
        current.copyWith(
          devices: latestRemainingDevices,
          rooms: latestRemainingRooms,
          operations: latestOperations,
        ),
      );
    });
  }

  Future<void> _removeRoom(String roomId) async {
    _stateRevision++;
    await _serializeMutation(() async {
      await ref.read(displayCacheProvider).removeRoomCascade(roomId);
      final current = state.asData?.value;
      if (current == null) return;
      final nextOperations = Map<String, DisconnectOperation>.from(
        current.operations,
      )..remove(_operationKey(DisconnectKind.room, roomId));
      state = AsyncData(
        current.copyWith(
          rooms: current.rooms
              .where((room) => room['id'] != roomId)
              .toList(growable: false),
          operations: nextOperations,
        ),
      );
    });
  }

  Future<T> _serializeMutation<T>(Future<T> Function() mutation) {
    final completer = Completer<T>();
    _mutationTail = _mutationTail.then((_) async {
      try {
        completer.complete(await mutation());
      } catch (error, stackTrace) {
        completer.completeError(error, stackTrace);
      }
    });
    return completer.future;
  }

  void _setOperation(DisconnectOperation operation) {
    final current = state.asData?.value;
    if (current == null) return;
    final operations = Map<String, DisconnectOperation>.from(
      current.operations,
    );
    operations[_operationKey(operation.kind, operation.aggregateId)] =
        operation;
    state = AsyncData(current.copyWith(operations: operations));
  }
}

String _operationKey(DisconnectKind kind, String aggregateId) =>
    '${kind.name}:$aggregateId';

bool _isAmbiguousDisconnectError(Object error) {
  if (error is TimeoutException) return true;
  if (error is! DioException) return false;
  if (error.response?.statusCode == 404) return true;
  return switch (error.type) {
    DioExceptionType.connectionError ||
    DioExceptionType.connectionTimeout ||
    DioExceptionType.receiveTimeout ||
    DioExceptionType.sendTimeout => true,
    _ => false,
  };
}
