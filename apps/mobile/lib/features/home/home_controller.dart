import 'package:dio/dio.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../core/network/api_client.dart';
import '../../core/sync/mutation_queue.dart';
import '../../storage/display_cache.dart';
import '../auth/connection_gate_controller.dart';

final homeControllerProvider =
    AsyncNotifierProvider.autoDispose<HomeController, HomeData>(
      HomeController.new,
    );

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
}

class HomeController extends AsyncNotifier<HomeData> {
  @override
  Future<HomeData> build() async {
    final mutationQueue = ref.read(mutationQueueProvider);
    var outbox = await mutationQueue.summary();
    final api = ref.watch(apiClientProvider);
    final cache = ref.watch(displayCacheProvider);
    final gate = ref.watch(connectionGateControllerProvider).requireValue;
    final gateDeviceIds = _ids(gate.devices);
    final gateRoomIds = _ids(gate.rooms);
    try {
      final results = await Future.wait([
        api.getList('/v1/devices'),
        api.getList('/v1/rooms'),
      ]);
      final baseDevices = results[0]
          .where(isActiveConnectionItem)
          .where((item) => gateDeviceIds.contains(item['id']))
          .toList(growable: false);
      final baseRooms = results[1]
          .where(isActiveConnectionItem)
          .where((item) => gateRoomIds.contains(item['id']))
          .toList(growable: false);
      final currentGate = ref
          .read(connectionGateControllerProvider)
          .asData
          ?.value;
      final connectionUnchanged =
          currentGate != null &&
          gate.operations.isEmpty &&
          currentGate.operations.isEmpty &&
          _sameIds(gateDeviceIds, _ids(currentGate.devices)) &&
          _sameIds(gateRoomIds, _ids(currentGate.rooms)) &&
          _sameIds(gateDeviceIds, _ids(baseDevices)) &&
          _sameIds(gateRoomIds, _ids(baseRooms));
      if (connectionUnchanged) {
        await mutationQueue.flush();
      } else {
        // Let the gate replace caches and reset navigation. Home never applies
        // a connection list that is older than the gate snapshot it started on.
        await ref.read(connectionGateControllerProvider.notifier).reconcile();
      }
      outbox = await mutationQueue.summary();
      final devices = await Future.wait(
        baseDevices.map((device) async {
          final presence = await api.get(
            '/v1/devices/${device['id']}/presence',
          );
          return {...device, 'presence': presence['presence']};
        }),
      );
      final rooms = await Future.wait(
        baseRooms.map((room) async {
          final roomId = room['id'] as String;
          final details = await Future.wait<dynamic>([
            api.getList('/v1/rooms/$roomId/proposals/open'),
            api.getList('/v1/rooms/$roomId/executions'),
            api.getNullable('/v1/rooms/$roomId/snapshots/latest'),
          ]);
          final executions = details[1] as List<Map<String, dynamic>>;
          final latestExecution = executions.isEmpty
              ? null
              : Map<String, dynamic>.from(executions.first['execution'] as Map);
          final snapshot = details[2] as Map<String, dynamic>?;
          return {
            ...room,
            'pendingProposalCount':
                (details[0] as List<Map<String, dynamic>>).length,
            'latestExecutionStatus': latestExecution?['status'],
            'cleanlinessScore': snapshot?['score'],
            'cleanlinessFormulaVersion': snapshot?['formulaVersion'],
            'cleanlinessCalculatedAt': snapshot?['calculatedAt'],
          };
        }),
      );
      final character = await api.get('/v1/character');
      return HomeData(
        devices: devices,
        rooms: rooms,
        isOffline: false,
        outboxPending: outbox.pending,
        outboxFailed: outbox.failed,
        character: character,
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

  Future<void> reload() async {
    ref.invalidateSelf();
    await future;
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
