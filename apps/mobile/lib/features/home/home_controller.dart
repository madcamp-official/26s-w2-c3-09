import 'dart:async';

import 'package:dio/dio.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/network/api_client.dart';
import '../../core/sync/mutation_queue.dart';
import '../../core/sync/realtime_controller.dart';
import '../../storage/display_cache.dart';
import '../auth/connection_gate_controller.dart';

const homeSummaryPath = '/v1/home/summary';

typedef HomeSummaryFetcher = Future<Map<String, dynamic>> Function();

final homeSummaryFetcherProvider = Provider<HomeSummaryFetcher>((ref) {
  final api = ref.watch(apiClientProvider);
  return () => api.get(homeSummaryPath);
});

final homeControllerProvider =
    AsyncNotifierProvider.autoDispose<HomeController, HomeData>(
      HomeController.new,
    );

class HomeSummaryPayload {
  const HomeSummaryPayload({
    required this.devices,
    required this.rooms,
    required this.character,
  });

  factory HomeSummaryPayload.fromJson(Map<String, dynamic> value) {
    return HomeSummaryPayload(
      devices: _mapList(value, 'devices'),
      rooms: _mapList(value, 'rooms'),
      character: switch (value['character']) {
        null => null,
        final Map character => Map<String, dynamic>.from(character),
        _ => throw const FormatException('INVALID_HOME_SUMMARY_CHARACTER'),
      },
    );
  }

  final List<Map<String, dynamic>> devices;
  final List<Map<String, dynamic>> rooms;
  final Map<String, dynamic>? character;

  static List<Map<String, dynamic>> _mapList(
    Map<String, dynamic> value,
    String key,
  ) {
    final raw = value[key];
    if (raw is! List) {
      throw FormatException('INVALID_HOME_SUMMARY_${key.toUpperCase()}');
    }
    return raw
        .map((item) {
          if (item is! Map) {
            throw FormatException('INVALID_HOME_SUMMARY_${key.toUpperCase()}');
          }
          return Map<String, dynamic>.from(item);
        })
        .toList(growable: false);
  }
}

Future<HomeSummaryPayload> fetchHomeSummary(HomeSummaryFetcher fetch) async {
  return HomeSummaryPayload.fromJson(await fetch());
}

class HomeData {
  const HomeData({
    required this.devices,
    required this.rooms,
    required this.isOffline,
    required this.outboxPending,
    required this.outboxFailed,
    this.character,
  });

  final List<Map<String, dynamic>> devices;
  final List<Map<String, dynamic>> rooms;
  final bool isOffline;
  final int outboxPending;
  final int outboxFailed;
  final Map<String, dynamic>? character;

  HomeData copyWith({
    List<Map<String, dynamic>>? devices,
    List<Map<String, dynamic>>? rooms,
    bool? isOffline,
    int? outboxPending,
    int? outboxFailed,
    Map<String, dynamic>? character,
  }) {
    return HomeData(
      devices: devices ?? this.devices,
      rooms: rooms ?? this.rooms,
      isOffline: isOffline ?? this.isOffline,
      outboxPending: outboxPending ?? this.outboxPending,
      outboxFailed: outboxFailed ?? this.outboxFailed,
      character: character ?? this.character,
    );
  }
}

List<Map<String, dynamic>> mergeHomeSummaryConnectionItems({
  required List<Map<String, dynamic>> authoritative,
  required List<Map<String, dynamic>> enriched,
}) {
  final enrichedById = <String, Map<String, dynamic>>{
    for (final item in enriched)
      if (item['id'] is String) item['id'] as String: item,
  };
  return authoritative
      .map((item) {
        final id = item['id'];
        return id is String ? {...item, ...?enrichedById[id]} : {...item};
      })
      .toList(growable: false);
}

HomeData? reduceHomeDataForRealtimeUpdate({
  required HomeData current,
  required RealtimeHomeUpdate update,
  required Set<String> activeDeviceIds,
  required Set<String> activeRoomIds,
}) {
  switch (update.kind) {
    case RealtimeHomeUpdateKind.refreshSummary:
      return null;
    case RealtimeHomeUpdateKind.presence:
      final deviceId = update.deviceId;
      final presence = update.presence;
      if (deviceId == null ||
          presence == null ||
          !activeDeviceIds.contains(deviceId)) {
        return current;
      }
      var changed = false;
      final devices = current.devices
          .map((device) {
            if (device['id'] != deviceId || device['presence'] == presence) {
              return device;
            }
            changed = true;
            return {...device, 'presence': presence};
          })
          .toList(growable: false);
      return changed ? current.copyWith(devices: devices) : current;
    case RealtimeHomeUpdateKind.deviceRemoved:
      final deviceId = update.deviceId;
      if (deviceId == null) return current;
      final devices = current.devices
          .where((device) => device['id'] != deviceId)
          .toList(growable: false);
      final rooms = current.rooms
          .where((room) => room['desktopDeviceId'] != deviceId)
          .toList(growable: false);
      if (devices.length == current.devices.length &&
          rooms.length == current.rooms.length) {
        return current;
      }
      return current.copyWith(devices: devices, rooms: rooms);
    case RealtimeHomeUpdateKind.roomRemoved:
      final roomId = update.roomId;
      if (roomId == null) return current;
      final rooms = current.rooms
          .where((room) => room['id'] != roomId)
          .toList(growable: false);
      return rooms.length == current.rooms.length
          ? current
          : current.copyWith(rooms: rooms);
    case RealtimeHomeUpdateKind.proposalCreated:
    case RealtimeHomeUpdateKind.decisionCreated:
      final roomId = update.roomId;
      final pendingProposalCount = update.pendingProposalCount;
      if (roomId == null || !activeRoomIds.contains(roomId)) {
        return current;
      }
      if (pendingProposalCount == null) return null;
      var changed = false;
      final rooms = current.rooms
          .map((room) {
            if (room['id'] != roomId ||
                room['pendingProposalCount'] == pendingProposalCount) {
              return room;
            }
            changed = true;
            return {...room, 'pendingProposalCount': pendingProposalCount};
          })
          .toList(growable: false);
      return changed ? current.copyWith(rooms: rooms) : current;
    case RealtimeHomeUpdateKind.roomSnapshotUpdated:
      final roomId = update.roomId;
      final snapshot = update.roomSnapshot;
      if (roomId == null ||
          snapshot == null ||
          !activeRoomIds.contains(roomId)) {
        return current;
      }
      var changed = false;
      final rooms = current.rooms
          .map((room) {
            if (room['id'] != roomId ||
                !_snapshotIsNewer(
                  currentCalculatedAt: room['cleanlinessCalculatedAt'],
                  nextCalculatedAt: snapshot['calculatedAt'],
                )) {
              return room;
            }
            changed = true;
            return {
              ...room,
              'cleanlinessScore': snapshot['score'],
              'cleanlinessFormulaVersion': snapshot['formulaVersion'],
              'cleanlinessCalculatedAt': snapshot['calculatedAt'],
            };
          })
          .toList(growable: false);
      return changed ? current.copyWith(rooms: rooms) : current;
    case RealtimeHomeUpdateKind.commandStatus:
      return current;
    case RealtimeHomeUpdateKind.executionStatus:
      final roomId = update.roomId;
      final status = update.executionStatus;
      if (roomId == null || status == null || !activeRoomIds.contains(roomId)) {
        return current;
      }
      var changed = false;
      final rooms = current.rooms
          .map((room) {
            if (room['id'] != roomId ||
                room['latestExecutionStatus'] == status) {
              return room;
            }
            changed = true;
            return {...room, 'latestExecutionStatus': status};
          })
          .toList(growable: false);
      return changed ? current.copyWith(rooms: rooms) : current;
  }
}

bool _snapshotIsNewer({
  required Object? currentCalculatedAt,
  required Object? nextCalculatedAt,
}) {
  if (nextCalculatedAt is! String || nextCalculatedAt.isEmpty) return false;
  final next = DateTime.tryParse(nextCalculatedAt);
  if (next == null) return false;
  if (currentCalculatedAt is! String || currentCalculatedAt.isEmpty) {
    return true;
  }
  final current = DateTime.tryParse(currentCalculatedAt);
  return current == null || next.isAfter(current);
}

class HomeController extends AsyncNotifier<HomeData> {
  Future<HomeData>? _loadInFlight;

  @override
  Future<HomeData> build() async {
    ref.watch(homeSummaryFetcherProvider);
    ref.watch(mutationQueueProvider);
    ref.watch(displayCacheProvider);
    // The five-second connection safety reconcile owns the authoritative
    // device/room gate. Home data should not refetch just because that gate
    // confirmed liveness; summary reloads are reserved for explicit refreshes
    // and realtime events without a complete targeted patch.
    final gate = ref.read(connectionGateControllerProvider).requireValue;
    ref.listen(realtimeHomeUpdateProvider, (previous, next) {
      if (next == null || identical(previous, next)) return;
      unawaited(applyRealtimeUpdate(next));
    });
    return _loadOnce(gate);
  }

  Future<HomeData> _loadOnce(ConnectionGateData gateAtStart) {
    final existing = _loadInFlight;
    if (existing != null) return existing;
    late final Future<HomeData> load;
    load = _loadHomeData(gateAtStart).whenComplete(() {
      if (identical(_loadInFlight, load)) _loadInFlight = null;
    });
    _loadInFlight = load;
    return load;
  }

  Future<HomeData> _loadHomeData(ConnectionGateData gateAtStart) async {
    final mutationQueue = ref.read(mutationQueueProvider);
    var outbox = await mutationQueue.summary();
    final cache = ref.read(displayCacheProvider);
    final gateDeviceIds = _ids(gateAtStart.devices);
    final gateRoomIds = _ids(gateAtStart.rooms);
    try {
      final summary = await fetchHomeSummary(
        ref.read(homeSummaryFetcherProvider),
      );
      final summaryDevices = summary.devices
          .where(isActiveConnectionItem)
          .toList(growable: false);
      final summaryRooms = summary.rooms
          .where(isActiveConnectionItem)
          .toList(growable: false);
      final currentGate =
          ref.read(connectionGateControllerProvider).asData?.value ??
          gateAtStart;
      final currentDeviceIds = _ids(currentGate.devices);
      final currentRoomIds = _ids(currentGate.rooms);
      final devices = mergeHomeSummaryConnectionItems(
        authoritative: currentGate.devices,
        enriched: summaryDevices,
      );
      final rooms = mergeHomeSummaryConnectionItems(
        authoritative: currentGate.rooms,
        enriched: summaryRooms,
      );

      // This update-only cache path cannot recreate lifecycle tombstones even
      // when an older summary response arrives after a revoke/remove event.
      await cache.enrichConnectionState(devices: devices, rooms: rooms);

      final connectionUnchanged =
          gateAtStart.operations.isEmpty &&
          currentGate.operations.isEmpty &&
          _sameIds(gateDeviceIds, currentDeviceIds) &&
          _sameIds(gateRoomIds, currentRoomIds) &&
          _sameIds(currentDeviceIds, _ids(summaryDevices)) &&
          _sameIds(currentRoomIds, _ids(summaryRooms));
      if (connectionUnchanged) await mutationQueue.flush();
      outbox = await mutationQueue.summary();
      return HomeData(
        devices: devices,
        rooms: rooms,
        isOffline: false,
        outboxPending: outbox.pending,
        outboxFailed: outbox.failed,
        character: summary.character,
      );
    } on DioException catch (error) {
      if (!_isOffline(error)) rethrow;
      final cached = await Future.wait([cache.devices(), cache.rooms()]);
      if (cached[0].isEmpty && cached[1].isEmpty) rethrow;
      return HomeData(
        devices: cached[0],
        rooms: cached[1],
        isOffline: true,
        outboxPending: outbox.pending,
        outboxFailed: outbox.failed,
      );
    }
  }

  Set<String> _ids(Iterable<Map<String, dynamic>> values) =>
      values.map((value) => value['id']).whereType<String>().toSet();

  bool _sameIds(Set<String> left, Set<String> right) =>
      left.length == right.length && left.containsAll(right);

  Future<void> reload({bool preserveCurrentOnError = false}) async {
    final previous = state;
    final gate = ref.read(connectionGateControllerProvider).requireValue;
    try {
      state = AsyncData(await _loadOnce(gate));
    } catch (error, stackTrace) {
      if (preserveCurrentOnError && previous.hasValue) return;
      state = AsyncError(error, stackTrace);
      rethrow;
    }
  }

  Future<void> applyRealtimeUpdate(RealtimeHomeUpdate update) async {
    final current = state.asData?.value;
    if (current == null) {
      if (update.kind == RealtimeHomeUpdateKind.refreshSummary) {
        await reload(preserveCurrentOnError: true);
      }
      return;
    }
    final gate = ref.read(connectionGateControllerProvider).asData?.value;
    final patched = reduceHomeDataForRealtimeUpdate(
      current: current,
      update: update,
      activeDeviceIds: _ids(gate?.devices ?? const []),
      activeRoomIds: _ids(gate?.rooms ?? const []),
    );
    if (patched == null) {
      await reload(preserveCurrentOnError: true);
    } else if (!identical(patched, current)) {
      state = AsyncData(patched);
    }
  }

  Future<void> revokeDevice(String deviceId) async {
    await ref.read(apiClientProvider).delete('/v1/devices/$deviceId');
    await reload();
  }

  Future<void> discardFailedMutations() async {
    await ref.read(mutationQueueProvider).discardFailed();
    await reload();
  }

  bool _isOffline(DioException error) {
    return switch (error.type) {
      DioExceptionType.connectionError ||
      DioExceptionType.connectionTimeout ||
      DioExceptionType.receiveTimeout ||
      DioExceptionType.sendTimeout => true,
      _ => false,
    };
  }
}
