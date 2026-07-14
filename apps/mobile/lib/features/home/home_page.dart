import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../core/notifications/push_notifications.dart';
import '../../core/models/character_state.dart';
import '../../core/sync/realtime_controller.dart';
import '../auth/auth_controller.dart';
import '../auth/connection_gate_controller.dart';
import '../character/character_settings_page.dart';
import '../character/mousekeeper_motion.dart';
import '../files/files_page.dart';
import '../rooms/room_page.dart';
import 'home_controller.dart';

List<Map<String, dynamic>> mergeAuthoritativeConnectionItems({
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

/// A lost WebSocket lifecycle event is repaired by an authoritative REST
/// check. Presence itself is delivered as a targeted realtime patch, so this
/// five-second safety loop never needs to poll the full home projection.
const homeAuthoritativeReconcileInterval = Duration(seconds: 5);

class HomeAuthoritativeReconcileLoop {
  HomeAuthoritativeReconcileLoop({
    required this.reconcile,
    this.interval = homeAuthoritativeReconcileInterval,
  });

  final Future<bool> Function() reconcile;
  final Duration interval;

  Timer? _timer;
  bool _inFlight = false;
  bool _disposed = false;

  void start() {
    if (_disposed || _timer != null) return;
    _timer = Timer.periodic(interval, (_) => unawaited(_tick()));
  }

  Future<void> _tick() async {
    if (_disposed || _inFlight) return;
    _inFlight = true;
    try {
      await reconcile();
    } catch (_) {
      // This is a background fail-closed repair path. The next five-second
      // tick retries without replacing the last verified connection state.
    } finally {
      _inFlight = false;
    }
  }

  void dispose() {
    _disposed = true;
    _timer?.cancel();
    _timer = null;
  }
}

class HomePage extends ConsumerStatefulWidget {
  const HomePage({super.key});

  @override
  ConsumerState<HomePage> createState() => _HomePageState();
}

class _HomePageState extends ConsumerState<HomePage> {
  late final HomeAuthoritativeReconcileLoop _reconcileLoop;

  @override
  void initState() {
    super.initState();
    unawaited(ref.read(realtimeRevisionProvider.notifier).connect());
    _reconcileLoop = HomeAuthoritativeReconcileLoop(
      reconcile: () =>
          ref.read(connectionGateControllerProvider.notifier).reconcile(),
    )..start();
  }

  @override
  void dispose() {
    _reconcileLoop.dispose();
    ref.read(realtimeRevisionProvider.notifier).disconnect();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(homeControllerProvider);
    final pushNotifications = ref.watch(pushNotificationsProvider);
    final realtimeCharacterKind = ref.watch(realtimeCharacterKindProvider);
    final gateData = ref.watch(connectionGateControllerProvider).asData?.value;
    return Scaffold(
      appBar: AppBar(
        title: const Text('MOUSEKEEPER'),
        actions: [
          IconButton(onPressed: _signOut, icon: const Icon(Icons.logout)),
        ],
      ),
      body: state.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (error, _) => HomeConnectionError(
          error: error,
          onRetry: () => ref.invalidate(homeControllerProvider),
        ),
        data: (data) {
          final devices = mergeAuthoritativeConnectionItems(
            authoritative: gateData?.devices ?? const [],
            enriched: data.devices,
          );
          final rooms = mergeAuthoritativeConnectionItems(
            authoritative: gateData?.rooms ?? const [],
            enriched: data.rooms,
          );
          final disconnecting =
              gateData?.operations.values.any(
                (operation) => operation.phase == DisconnectPhase.disconnecting,
              ) ??
              false;
          return RefreshIndicator(
            onRefresh: () => ref.read(homeControllerProvider.notifier).reload(),
            child: ListView(
              padding: const EdgeInsets.all(16),
              children: [
                PushNotificationStatusCard(state: pushNotifications),
                if (data.isOffline) ...[
                  const OfflineCacheBanner(),
                  const SizedBox(height: 12),
                ],
                if (data.outboxPending > 0 || data.outboxFailed > 0) ...[
                  Card(
                    color: data.outboxFailed > 0
                        ? const Color(0xFFFFEBEE)
                        : const Color(0xFFE3F2FD),
                    child: ListTile(
                      leading: Icon(
                        data.outboxFailed > 0
                            ? Icons.error_outline
                            : Icons.outbox_outlined,
                      ),
                      title: Text(
                        data.outboxFailed > 0
                            ? '전송하지 못한 요청 ${data.outboxFailed}건'
                            : '연결 후 전송할 요청 ${data.outboxPending}건',
                      ),
                      subtitle: Text(
                        data.outboxFailed > 0
                            ? '서버가 거절한 요청입니다. 상태를 확인한 뒤 목록에서 정리하세요.'
                            : '같은 idempotency key로 안전하게 다시 전송합니다.',
                      ),
                      trailing: data.outboxFailed > 0
                          ? TextButton(
                              onPressed: () => ref
                                  .read(homeControllerProvider.notifier)
                                  .discardFailedMutations(),
                              child: const Text('정리'),
                            )
                          : null,
                    ),
                  ),
                  const SizedBox(height: 12),
                ],
                Card(
                  child: ListTile(
                    leading: SizedBox(
                      width: 52,
                      height: 52,
                      child: MouseKeeperMotionImage(
                        motion: mousekeeperMotionForHome(
                          isOffline: data.isOffline,
                          presences: devices.map(
                            (item) => item['presence'] as String? ?? 'OFFLINE',
                          ),
                          executionStatuses: rooms.map(
                            (item) => item['latestExecutionStatus'] as String?,
                          ),
                          hasPendingProposal: rooms.any(
                            (item) =>
                                (item['pendingProposalCount'] as int? ?? 0) > 0,
                          ),
                          realtimeCharacterKind: disconnecting
                              ? CharacterState.connecting
                              : realtimeCharacterKind,
                        ),
                      ),
                    ),
                    title: const Text('MOUSEKEEPER 캐릭터'),
                    subtitle: disconnecting
                        ? const Text('연결 해제 결과를 확인하는 중')
                        : data.character == null
                        ? const Text('오프라인 · 캐릭터 설정을 확인할 수 없음')
                        : Text(
                            '호감도 ${data.character!['affinityTotal'] ?? 0} · '
                            '${data.character!['riveAssetStatus'] == 'UNCONFIGURED' ? 'PNG 상태 모션 사용 중 · Rive 미설정' : '모션 연결됨'}',
                          ),
                    trailing: data.character == null
                        ? null
                        : const Icon(Icons.info_outline),
                    onTap: data.character == null
                        ? null
                        : () async {
                            final changed = await Navigator.of(context)
                                .push<bool>(
                                  MaterialPageRoute(
                                    builder: (_) => CharacterSettingsPage(
                                      initialCharacter: data.character!,
                                    ),
                                  ),
                                );
                            if (changed == true) {
                              ref.invalidate(homeControllerProvider);
                            }
                          },
                  ),
                ),
                const SizedBox(height: 20),
                Text('내 PC', style: Theme.of(context).textTheme.titleLarge),
                const SizedBox(height: 8),
                ...devices.map((item) {
                  final presence = item['presence'] as String? ?? 'OFFLINE';
                  final online = presence.startsWith('ONLINE');
                  final operation = gateData?.operation(
                    DisconnectKind.device,
                    item['id'] as String,
                  );
                  return Card(
                    child: Column(
                      children: [
                        ListTile(
                          leading: Icon(
                            online ? Icons.lightbulb : Icons.lightbulb_outline,
                            color: online ? Colors.amber.shade700 : Colors.grey,
                          ),
                          title: Text(item['deviceName'] as String? ?? 'PC'),
                          subtitle: Text(
                            operation?.phase == DisconnectPhase.disconnecting
                                ? '기기 연결을 해제하는 중'
                                : online
                                ? _presenceLabel(presence)
                                : 'PC 에이전트와 연결되지 않음',
                          ),
                          trailing: IconButton(
                            tooltip: '기기 연결 해제',
                            icon:
                                operation?.phase ==
                                    DisconnectPhase.disconnecting
                                ? const SizedBox.square(
                                    dimension: 20,
                                    child: CircularProgressIndicator(
                                      strokeWidth: 2,
                                    ),
                                  )
                                : const Icon(Icons.link_off),
                            onPressed: operation == null
                                ? () => _confirmRevoke(
                                    item['id'] as String,
                                    item['deviceName'] as String? ?? 'PC',
                                  )
                                : null,
                          ),
                        ),
                        if (operation?.phase == DisconnectPhase.failed)
                          DisconnectFailurePanel(
                            message: operation!.message,
                            onRetry: () => ref
                                .read(connectionGateControllerProvider.notifier)
                                .retryDisconnect(
                                  DisconnectKind.device,
                                  item['id'] as String,
                                ),
                          ),
                      ],
                    ),
                  );
                }),
                const SizedBox(height: 20),
                Text('내 방', style: Theme.of(context).textTheme.titleLarge),
                const SizedBox(height: 8),
                if (rooms.isEmpty)
                  const EmptyRoomsCard()
                else
                  ...rooms.map((item) {
                    final roomId = item['id'] as String;
                    final operation = gateData?.operation(
                      DisconnectKind.room,
                      roomId,
                    );
                    return Card(
                      child: Column(
                        children: [
                          ListTile(
                            leading: Badge(
                              isLabelVisible:
                                  (item['pendingProposalCount'] as int? ?? 0) >
                                  0,
                              label: Text(
                                '${item['pendingProposalCount'] ?? 0}',
                              ),
                              child: const Icon(Icons.meeting_room_outlined),
                            ),
                            title: Text(item['name'] as String? ?? '방'),
                            subtitle: Text(
                              operation?.phase == DisconnectPhase.disconnecting
                                  ? '폴더 연결을 해제하는 중'
                                  : '${_rootAliasLabel(item['rootAlias'])}'
                                        '${_homeCleanlinessLabel(item)}'
                                        '${item['latestExecutionStatus'] == null ? '' : ' · 최근 ${item['latestExecutionStatus']}'}',
                            ),
                            trailing: Row(
                              mainAxisSize: MainAxisSize.min,
                              children: [
                                IconButton(
                                  tooltip: '파일 열기',
                                  onPressed: operation == null
                                      ? () => Navigator.of(context).push(
                                          MaterialPageRoute(
                                            builder: (_) => FilesPage(
                                              roomId: roomId,
                                              roomName:
                                                  item['name'] as String? ??
                                                  '방',
                                            ),
                                          ),
                                        )
                                      : null,
                                  icon: const Icon(Icons.folder_open),
                                ),
                                IconButton(
                                  tooltip: '폴더 연결 해제',
                                  onPressed: operation == null
                                      ? () => _confirmRemoveRoom(
                                          roomId,
                                          item['name'] as String? ?? '방',
                                        )
                                      : null,
                                  icon:
                                      operation?.phase ==
                                          DisconnectPhase.disconnecting
                                      ? const SizedBox.square(
                                          dimension: 20,
                                          child: CircularProgressIndicator(
                                            strokeWidth: 2,
                                          ),
                                        )
                                      : const Icon(Icons.link_off),
                                ),
                              ],
                            ),
                            onTap: operation == null
                                ? () => Navigator.of(context).push(
                                    MaterialPageRoute(
                                      builder: (_) => RoomPage(room: item),
                                    ),
                                  )
                                : null,
                          ),
                          if (operation?.phase == DisconnectPhase.failed)
                            DisconnectFailurePanel(
                              message: operation!.message,
                              onRetry: () => ref
                                  .read(
                                    connectionGateControllerProvider.notifier,
                                  )
                                  .retryDisconnect(DisconnectKind.room, roomId),
                            ),
                        ],
                      ),
                    );
                  }),
              ],
            ),
          );
        },
      ),
    );
  }

  String _presenceLabel(String presence) {
    return switch (presence) {
      'ONLINE_SCANNING' => 'PC가 폴더를 스캔 중',
      'ONLINE_EXECUTING' => 'PC가 승인된 작업을 실행 중',
      'DEGRADED' => 'PC 연결 상태가 불안정함',
      _ => 'PC 연결됨',
    };
  }

  Future<void> _signOut() async {
    try {
      await ref.read(pushNotificationsProvider.notifier).unregister();
      await ref.read(authControllerProvider.notifier).signOut();
    } catch (error) {
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text('로그아웃 준비 실패: $error')));
    }
  }

  String _rootAliasLabel(Object? value) {
    final alias = value is String ? value : '';
    return alias.startsWith('root:') || alias.isEmpty ? '관리 폴더 연결됨' : alias;
  }

  Future<void> _confirmRevoke(String deviceId, String deviceName) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('기기 연결 해제'),
        content: Text('$deviceName의 device token을 즉시 무효화합니다.'),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('취소'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('연결 해제'),
          ),
        ],
      ),
    );
    if (confirmed != true) return;
    await ref
        .read(connectionGateControllerProvider.notifier)
        .disconnectDevice(deviceId);
  }

  String _homeCleanlinessLabel(Map<String, dynamic> room) {
    final score = room['cleanlinessScore'];
    if (score == null) return '';
    final formulaVersion = room['cleanlinessFormulaVersion'];
    if (formulaVersion is String &&
        formulaVersion != supportedCleanlinessFormulaVersion) {
      return ' · 청결도 업데이트 필요';
    }
    return ' · 청결도 $score';
  }

  Future<void> _confirmRemoveRoom(String roomId, String roomName) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('폴더 연결 해제'),
        content: Text('$roomName 연결을 해제합니다. PC의 원본 폴더와 파일은 삭제되지 않습니다.'),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('취소'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('연결 해제'),
          ),
        ],
      ),
    );
    if (confirmed != true) return;
    await ref
        .read(connectionGateControllerProvider.notifier)
        .disconnectRoom(roomId);
  }
}

class PushNotificationStatusCard extends StatelessWidget {
  const PushNotificationStatusCard({super.key, required this.state});

  final AsyncValue<PushNotificationRegistration> state;

  @override
  Widget build(BuildContext context) => state.when(
    loading: () => const Card(
      child: ListTile(
        leading: CircularProgressIndicator(),
        title: Text('알림 연결 중'),
      ),
    ),
    error: (error, _) => Card(
      color: const Color(0xFFFFEBEE),
      child: ListTile(
        leading: const Icon(Icons.notifications_off_outlined),
        title: const Text('푸시 알림을 연결하지 못했어요'),
        subtitle: Text('$error'),
      ),
    ),
    data: (registration) => switch (registration.status) {
      PushNotificationStatus.active => const SizedBox.shrink(),
      PushNotificationStatus.permissionDenied => const Card(
        color: Color(0xFFFFF3E0),
        child: ListTile(
          leading: Icon(Icons.notifications_off_outlined),
          title: Text('알림 권한이 꺼져 있어요'),
          subtitle: Text('휴대폰 설정에서 MOUSEKEEPER 알림을 허용해 주세요.'),
        ),
      ),
      PushNotificationStatus.unconfigured => Card(
        color: const Color(0xFFFFF3E0),
        child: ListTile(
          leading: const Icon(Icons.notifications_off_outlined),
          title: const Text('푸시 알림 미설정'),
          subtitle: Text(registration.errorCode ?? 'UNCONFIGURED'),
        ),
      ),
    },
  );
}

class HomeConnectionError extends StatelessWidget {
  const HomeConnectionError({
    super.key,
    required this.error,
    required this.onRetry,
  });
  final Object error;
  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) => Center(
    child: Padding(
      padding: const EdgeInsets.all(24),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          const Icon(Icons.cloud_off_outlined, size: 48),
          const SizedBox(height: 12),
          Text('서버와 연결되지 않았습니다.\n$error', textAlign: TextAlign.center),
          const SizedBox(height: 12),
          FilledButton(onPressed: onRetry, child: const Text('다시 시도')),
        ],
      ),
    ),
  );
}

class OfflineCacheBanner extends StatelessWidget {
  const OfflineCacheBanner({super.key});

  @override
  Widget build(BuildContext context) => const Card(
    color: Color(0xFFFFF3E0),
    child: ListTile(
      leading: Icon(Icons.cloud_off_outlined),
      title: Text('오프라인 표시 데이터'),
      subtitle: Text('마지막으로 동기화된 정보를 표시합니다.'),
    ),
  );
}

class DisconnectFailurePanel extends StatelessWidget {
  const DisconnectFailurePanel({
    super.key,
    required this.message,
    required this.onRetry,
  });

  final String? message;
  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) => Container(
    width: double.infinity,
    color: Theme.of(context).colorScheme.errorContainer,
    padding: const EdgeInsets.fromLTRB(16, 8, 8, 8),
    child: Row(
      children: [
        const Icon(Icons.error_outline),
        const SizedBox(width: 8),
        Expanded(child: Text(message ?? '연결 해제에 실패했습니다.')),
        TextButton(onPressed: onRetry, child: const Text('다시 시도')),
      ],
    ),
  );
}

class EmptyRoomsCard extends StatelessWidget {
  const EmptyRoomsCard({super.key});

  @override
  Widget build(BuildContext context) => const Card(
    child: ListTile(
      leading: Icon(Icons.meeting_room_outlined),
      title: Text('연결된 폴더 없음'),
      subtitle: Text('PC에서 관리 폴더를 등록하면 여기에 표시됩니다.'),
    ),
  );
}
