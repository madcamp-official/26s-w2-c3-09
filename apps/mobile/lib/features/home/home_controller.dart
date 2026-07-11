import 'package:dio/dio.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../core/network/api_client.dart';
import '../../core/sync/mutation_queue.dart';
import '../../storage/display_cache.dart';

final homeControllerProvider = AsyncNotifierProvider<HomeController, HomeData>(
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
    await ref.read(mutationQueueProvider).flush();
    final outbox = await ref.read(mutationQueueProvider).summary();
    final api = ref.watch(apiClientProvider);
    final cache = ref.watch(displayCacheProvider);
    try {
      final results = await Future.wait([
        api.getList('/v1/devices'),
        api.getList('/v1/rooms'),
      ]);
      final devices = await Future.wait(
        results[0].map((device) async {
          final presence = await api.get(
            '/v1/devices/${device['id']}/presence',
          );
          return {...device, 'presence': presence['presence']};
        }),
      );
      final rooms = await Future.wait(
        results[1].map((room) async {
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
          };
        }),
      );
      final character = await api.get('/v1/character');
      await Future.wait([
        cache.replaceDevices(devices),
        cache.replaceRooms(rooms),
      ]);
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
