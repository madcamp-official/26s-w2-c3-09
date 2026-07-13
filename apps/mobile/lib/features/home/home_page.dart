import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../core/sync/realtime_controller.dart';
import '../auth/auth_controller.dart';
import '../auth/pairing_page.dart';
import '../character/character_settings_page.dart';
import '../character/housemouse_motion.dart';
import '../rooms/room_page.dart';
import 'home_controller.dart';

class HomePage extends ConsumerStatefulWidget {
  const HomePage({super.key});

  @override
  ConsumerState<HomePage> createState() => _HomePageState();
}

class _HomePageState extends ConsumerState<HomePage> {
  Timer? _presenceTimer;

  @override
  void initState() {
    super.initState();
    unawaited(ref.read(realtimeRevisionProvider.notifier).connect());
    _presenceTimer = Timer.periodic(const Duration(seconds: 15), (_) {
      ref.invalidate(homeControllerProvider);
    });
  }

  @override
  void dispose() {
    _presenceTimer?.cancel();
    ref.read(realtimeRevisionProvider.notifier).disconnect();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    ref.listen(realtimeRevisionProvider, (previous, next) {
      if (previous != null) ref.invalidate(homeControllerProvider);
    });
    final state = ref.watch(homeControllerProvider);
    final realtimeCharacterKind = ref.watch(realtimeCharacterKindProvider);
    return Scaffold(
      appBar: AppBar(
        title: const Text('HOUSEMOUSE'),
        actions: [
          IconButton(
            onPressed: () =>
                ref.read(authControllerProvider.notifier).signOut(),
            icon: const Icon(Icons.logout),
          ),
        ],
      ),
      body: state.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (error, _) => HomeConnectionError(
          error: error,
          onRetry: () => ref.invalidate(homeControllerProvider),
        ),
        data: (data) => RefreshIndicator(
          onRefresh: () => ref.read(homeControllerProvider.notifier).reload(),
          child: ListView(
            padding: const EdgeInsets.all(16),
            children: [
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
                    child: HousemouseMotionImage(
                      motion: housemouseMotionForHome(
                        isOffline: data.isOffline,
                        presences: data.devices.map(
                          (item) => item['presence'] as String? ?? 'OFFLINE',
                        ),
                        executionStatuses: data.rooms.map(
                          (item) => item['latestExecutionStatus'] as String?,
                        ),
                        hasPendingProposal: data.rooms.any(
                          (item) =>
                              (item['pendingProposalCount'] as int? ?? 0) > 0,
                        ),
                        realtimeCharacterKind: realtimeCharacterKind,
                      ),
                    ),
                  ),
                  title: const Text('HOUSEMOUSE 캐릭터'),
                  subtitle: data.character == null
                      ? const Text('오프라인 · 캐릭터 설정을 확인할 수 없음')
                      : Text(
                          '호감도 ${data.character!['affinityTotal'] ?? 0} · '
                          '${data.character!['riveAssetStatus'] == 'UNCONFIGURED' ? 'PNG 상태 모션 사용 중 · Rive 미설정' : '모션 연결됨'}',
                        ),
                  trailing: data.character == null
                      ? null
                      : const Icon(Icons.tune),
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
              if (data.devices.isEmpty)
                Card(
                  child: ListTile(
                    leading: const Icon(Icons.computer),
                    title: const Text('등록된 PC가 없습니다'),
                    subtitle: const Text('데스크톱 앱의 페어링 코드를 입력해 연결하세요.'),
                    trailing: const Icon(Icons.add_link),
                    onTap: () async {
                      final connected = await Navigator.of(context).push<bool>(
                        MaterialPageRoute(builder: (_) => const PairingPage()),
                      );
                      if (connected == true) {
                        ref.invalidate(homeControllerProvider);
                      }
                    },
                  ),
                )
              else
                ...data.devices.map((item) {
                  final presence = item['presence'] as String? ?? 'OFFLINE';
                  final online = presence.startsWith('ONLINE');
                  return Card(
                    child: ListTile(
                      leading: Icon(
                        online ? Icons.lightbulb : Icons.lightbulb_outline,
                        color: online ? Colors.amber.shade700 : Colors.grey,
                      ),
                      title: Text(item['deviceName'] as String? ?? 'PC'),
                      subtitle: Text(
                        online ? _presenceLabel(presence) : 'PC 에이전트와 연결되지 않음',
                      ),
                      trailing: IconButton(
                        tooltip: '기기 연결 해제',
                        icon: const Icon(Icons.link_off),
                        onPressed: () => _confirmRevoke(
                          item['id'] as String,
                          item['deviceName'] as String? ?? 'PC',
                        ),
                      ),
                    ),
                  );
                }),
              const SizedBox(height: 20),
              Text('내 방', style: Theme.of(context).textTheme.titleLarge),
              const SizedBox(height: 8),
              if (data.rooms.isEmpty)
                const EmptyRoomsCard()
              else
                ...data.rooms.map(
                  (item) => Card(
                    child: ListTile(
                      leading: Badge(
                        isLabelVisible:
                            (item['pendingProposalCount'] as int? ?? 0) > 0,
                        label: Text('${item['pendingProposalCount'] ?? 0}'),
                        child: const Icon(Icons.meeting_room_outlined),
                      ),
                      title: Text(item['name'] as String? ?? '방'),
                      subtitle: Text(
                        '${_rootAliasLabel(item['rootAlias'])}'
                        '${item['cleanlinessScore'] == null ? '' : ' · 청결도 ${item['cleanlinessScore']}'}'
                        '${item['latestExecutionStatus'] == null ? '' : ' · 최근 ${item['latestExecutionStatus']}'}',
                      ),
                      trailing: const Icon(Icons.chevron_right),
                      onTap: () => Navigator.of(context).push(
                        MaterialPageRoute(builder: (_) => RoomPage(room: item)),
                      ),
                    ),
                  ),
                ),
            ],
          ),
        ),
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
    try {
      await ref.read(homeControllerProvider.notifier).revokeDevice(deviceId);
    } catch (error) {
      if (mounted) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text('기기 연결 해제 실패: $error')));
      }
    }
  }
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

class EmptyRoomsCard extends StatelessWidget {
  const EmptyRoomsCard({super.key});

  @override
  Widget build(BuildContext context) => const Card(
    child: ListTile(
      leading: Icon(Icons.meeting_room_outlined),
      title: Text('등록된 방이 없습니다'),
      subtitle: Text('PC에서 관리 폴더를 등록하면 여기에 표시됩니다.'),
    ),
  );
}
