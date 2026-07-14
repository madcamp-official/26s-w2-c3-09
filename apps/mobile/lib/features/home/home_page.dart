import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:mousekeeper_character_assets/character_assets.dart';

import '../../core/models/character_state.dart';
import '../../core/notifications/push_notifications.dart';
import '../../core/sync/realtime_controller.dart';
import '../auth/auth_controller.dart';
import '../auth/connection_gate_controller.dart';
import '../character/mousekeeper_motion.dart';
import '../chat/chat_page.dart';
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

class _HomePageState extends ConsumerState<HomePage>
    with SingleTickerProviderStateMixin {
  late final HomeAuthoritativeReconcileLoop _reconcileLoop;
  late final AnimationController _backgroundController;
  Offset _mouseAlignment = const Offset(-0.12, 0.24);
  bool _mouseMenuOpen = false;
  String? _selectedRoomId;

  @override
  void initState() {
    super.initState();
    unawaited(ref.read(realtimeRevisionProvider.notifier).connect());
    _reconcileLoop = HomeAuthoritativeReconcileLoop(
      reconcile: () =>
          ref.read(connectionGateControllerProvider.notifier).reconcile(),
    )..start();
    _backgroundController = AnimationController(
      vsync: this,
      duration: const Duration(seconds: 42),
    )..repeat();
  }

  @override
  void dispose() {
    _backgroundController.dispose();
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
      extendBodyBehindAppBar: true,
      appBar: AppBar(
        backgroundColor: Colors.transparent,
        elevation: 0,
        title: const Text(
          'MOUSEKEEPER',
          style: TextStyle(fontWeight: FontWeight.w800),
        ),
        actions: [
          IconButton(
            tooltip: '로그아웃',
            onPressed: _signOut,
            icon: const Icon(Icons.logout),
          ),
        ],
      ),
      body: state.when(
        loading: () =>
            const _HomeStage(child: Center(child: CircularProgressIndicator())),
        error: (error, _) => _HomeStage(
          child: HomeConnectionError(
            error: error,
            onRetry: () => ref.invalidate(homeControllerProvider),
          ),
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
          final selectedRoom = _selectedRoom(rooms);
          final disconnecting =
              gateData?.operations.values.any(
                (operation) => operation.phase == DisconnectPhase.disconnecting,
              ) ??
              false;
          final motion = mousekeeperMotionForHome(
            isOffline: data.isOffline,
            presences: devices.map(
              (item) => item['presence'] as String? ?? 'OFFLINE',
            ),
            executionStatuses: rooms.map(
              (item) => item['latestExecutionStatus'] as String?,
            ),
            hasPendingProposal: rooms.any(
              (item) => (item['pendingProposalCount'] as int? ?? 0) > 0,
            ),
            realtimeCharacterKind: disconnecting
                ? CharacterState.connecting
                : realtimeCharacterKind,
          );
          return _HomeStage(
            controller: _backgroundController,
            onTapStage: _handleStageTap,
            dimBackground: _mouseMenuOpen,
            child: RefreshIndicator(
              onRefresh: () =>
                  ref.read(homeControllerProvider.notifier).reload(),
              child: LayoutBuilder(
                builder: (context, constraints) => SingleChildScrollView(
                  physics: const AlwaysScrollableScrollPhysics(),
                  child: ConstrainedBox(
                    constraints: BoxConstraints(
                      minHeight: constraints.maxHeight,
                    ),
                    child: Stack(
                      children: [
                        Positioned.fill(
                          child: IgnorePointer(
                            child: AnimatedOpacity(
                              opacity: _mouseMenuOpen ? 1 : 0,
                              duration: const Duration(milliseconds: 160),
                              child: Container(color: Colors.black54),
                            ),
                          ),
                        ),
                        Padding(
                          padding: const EdgeInsets.fromLTRB(18, 92, 18, 28),
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.stretch,
                            children: [
                              _ManagedFolderSelector(
                                rooms: rooms,
                                selectedRoomId: selectedRoom?['id'] as String?,
                                onChanged: (roomId) {
                                  setState(() => _selectedRoomId = roomId);
                                },
                                onOpenRoom: selectedRoom == null
                                    ? null
                                    : () => _openRoom(selectedRoom),
                              ),
                              const SizedBox(height: 16),
                              PushNotificationStatusCard(
                                state: pushNotifications,
                              ),
                              if (data.isOffline) ...[
                                const SizedBox(height: 12),
                                const OfflineCacheBanner(),
                              ],
                              if (data.outboxPending > 0 ||
                                  data.outboxFailed > 0) ...[
                                const SizedBox(height: 12),
                                _OutboxNotice(
                                  pending: data.outboxPending,
                                  failed: data.outboxFailed,
                                  onDiscardFailed: () => ref
                                      .read(homeControllerProvider.notifier)
                                      .discardFailedMutations(),
                                ),
                              ],
                              const SizedBox(height: 24),
                              SizedBox(
                                height: 360,
                                child: Stack(
                                  clipBehavior: Clip.none,
                                  children: [
                                    _MouseHomeActor(
                                      alignment: _mouseAlignment,
                                      motion: motion,
                                      menuOpen: _mouseMenuOpen,
                                      onTap: () => setState(
                                        () => _mouseMenuOpen = !_mouseMenuOpen,
                                      ),
                                    ),
                                    if (_mouseMenuOpen)
                                      _MouseSpeechBubble(
                                        selectedRoom: selectedRoom,
                                        onFiles: selectedRoom == null
                                            ? null
                                            : () => _openFiles(selectedRoom),
                                        onChat: selectedRoom == null
                                            ? null
                                            : () => _openChat(selectedRoom),
                                      ),
                                  ],
                                ),
                              ),
                              const SizedBox(height: 12),
                              _HomeActionGrid(
                                selectedRoom: selectedRoom,
                                onFiles: selectedRoom == null
                                    ? null
                                    : () => _openFiles(selectedRoom),
                                onChat: selectedRoom == null
                                    ? null
                                    : () => _openChat(selectedRoom),
                                onHistory: selectedRoom == null
                                    ? null
                                    : () => _openRoom(selectedRoom),
                              ),
                              const SizedBox(height: 16),
                              _ConnectionSummary(
                                devices: devices,
                                rooms: rooms,
                              ),
                            ],
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          );
        },
      ),
    );
  }

  void _handleStageTap(TapUpDetails details, Size size) {
    if (_mouseMenuOpen) {
      setState(() => _mouseMenuOpen = false);
      return;
    }
    final dx = (details.localPosition.dx / size.width) * 2 - 1;
    final dy = (details.localPosition.dy / size.height) * 2 - 1;
    setState(() {
      _mouseAlignment = Offset(
        dx.clamp(-0.72, 0.72).toDouble(),
        dy.clamp(-0.18, 0.58).toDouble(),
      );
    });
  }

  Map<String, dynamic>? _selectedRoom(List<Map<String, dynamic>> rooms) {
    if (rooms.isEmpty) return null;
    final selectedId = _selectedRoomId;
    if (selectedId != null) {
      for (final room in rooms) {
        if (room['id'] == selectedId) return room;
      }
    }
    return rooms.first;
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

  void _openRoom(Map<String, dynamic> room) {
    Navigator.of(
      context,
    ).push(MaterialPageRoute(builder: (_) => RoomPage(room: room)));
  }

  void _openFiles(Map<String, dynamic> room) {
    final roomId = room['id'] as String;
    final roomName = room['name'] as String? ?? '관리 폴더';
    Navigator.of(context).push(
      MaterialPageRoute(
        builder: (_) => FilesPage(roomId: roomId, roomName: roomName),
      ),
    );
  }

  void _openChat(Map<String, dynamic> room) {
    Navigator.of(context).push(
      MaterialPageRoute(builder: (_) => ChatPage(roomId: room['id'] as String)),
    );
  }
}

class _HomeStage extends StatelessWidget {
  const _HomeStage({
    required this.child,
    this.controller,
    this.onTapStage,
    this.dimBackground = false,
  });

  final Widget child;
  final AnimationController? controller;
  final void Function(TapUpDetails details, Size size)? onTapStage;
  final bool dimBackground;

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, constraints) {
      final size = constraints.biggest;
      return GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTapUp: onTapStage == null
            ? null
            : (details) => onTapStage!(details, size),
        child: Stack(
          fit: StackFit.expand,
          children: [
            _PanningHomeBackground(controller: controller),
            AnimatedContainer(
              duration: const Duration(milliseconds: 160),
              decoration: BoxDecoration(
                gradient: LinearGradient(
                  begin: Alignment.topCenter,
                  end: Alignment.bottomCenter,
                  colors: [
                    Colors.white.withValues(alpha: dimBackground ? 0.06 : 0.18),
                    const Color(
                      0xFFFFF5E9,
                    ).withValues(alpha: dimBackground ? 0.08 : 0.34),
                  ],
                ),
              ),
            ),
            child,
          ],
        ),
      );
    },
  );
}

class _PanningHomeBackground extends StatelessWidget {
  const _PanningHomeBackground({this.controller});

  final AnimationController? controller;

  @override
  Widget build(BuildContext context) {
    final animation = controller;
    if (animation == null) {
      return _backgroundFrame(0);
    }
    return AnimatedBuilder(
      animation: animation,
      builder: (context, _) => _backgroundFrame(animation.value),
    );
  }

  Widget _backgroundFrame(double value) {
    return Stack(
      fit: StackFit.expand,
      children: [
        for (
          var index = 0;
          index < mousekeeperHomeBackgroundAssets.length;
          index++
        )
          FractionalTranslation(
            translation: Offset(
              index - value * mousekeeperHomeBackgroundAssets.length,
              0,
            ),
            child: Image.asset(
              mousekeeperHomeBackgroundAssets[index],
              package: mousekeeperMascotPackage,
              fit: BoxFit.cover,
              filterQuality: FilterQuality.low,
            ),
          ),
        FractionalTranslation(
          translation: Offset(
            mousekeeperHomeBackgroundAssets.length -
                value * mousekeeperHomeBackgroundAssets.length,
            0,
          ),
          child: Image.asset(
            mousekeeperHomeBackgroundAssets.first,
            package: mousekeeperMascotPackage,
            fit: BoxFit.cover,
            filterQuality: FilterQuality.low,
          ),
        ),
      ],
    );
  }
}

class _ManagedFolderSelector extends StatelessWidget {
  const _ManagedFolderSelector({
    required this.rooms,
    required this.selectedRoomId,
    required this.onChanged,
    required this.onOpenRoom,
  });

  final List<Map<String, dynamic>> rooms;
  final String? selectedRoomId;
  final ValueChanged<String> onChanged;
  final VoidCallback? onOpenRoom;

  @override
  Widget build(BuildContext context) => Align(
    alignment: Alignment.centerRight,
    child: DecoratedBox(
      decoration: BoxDecoration(
        color: Colors.white.withValues(alpha: 0.88),
        borderRadius: BorderRadius.circular(22),
        border: Border.all(
          color: const Color(0xFF7E5C3F).withValues(alpha: 0.24),
        ),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.08),
            blurRadius: 14,
            offset: const Offset(0, 8),
          ),
        ],
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Icon(Icons.folder_special_outlined, size: 18),
            const SizedBox(width: 8),
            if (rooms.isEmpty)
              const Text('관리 폴더 없음')
            else
              DropdownButtonHideUnderline(
                child: DropdownButton<String>(
                  key: const ValueKey('managed-folder-selector'),
                  value: selectedRoomId,
                  borderRadius: BorderRadius.circular(16),
                  items: [
                    for (final room in rooms)
                      DropdownMenuItem<String>(
                        value: room['id'] as String?,
                        child: Text(
                          room['name'] as String? ?? '관리 폴더',
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                  ],
                  onChanged: (value) {
                    if (value != null) onChanged(value);
                  },
                ),
              ),
            IconButton(
              tooltip: '관리 폴더 상세',
              visualDensity: VisualDensity.compact,
              onPressed: onOpenRoom,
              icon: const Icon(Icons.tune_outlined, size: 18),
            ),
          ],
        ),
      ),
    ),
  );
}

class _MouseHomeActor extends StatelessWidget {
  const _MouseHomeActor({
    required this.alignment,
    required this.motion,
    required this.menuOpen,
    required this.onTap,
  });

  final Offset alignment;
  final MouseKeeperMotion motion;
  final bool menuOpen;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) => AnimatedAlign(
    duration: const Duration(milliseconds: 620),
    curve: Curves.easeOutBack,
    alignment: Alignment(alignment.dx, alignment.dy),
    child: GestureDetector(
      behavior: HitTestBehavior.translucent,
      onTap: onTap,
      child: AnimatedScale(
        scale: menuOpen ? 1.08 : 1,
        duration: const Duration(milliseconds: 180),
        child: MouseKeeperMotionImage(motion: motion, width: 176, height: 176),
      ),
    ),
  );
}

class _MouseSpeechBubble extends StatelessWidget {
  const _MouseSpeechBubble({
    required this.selectedRoom,
    required this.onFiles,
    required this.onChat,
  });

  final Map<String, dynamic>? selectedRoom;
  final VoidCallback? onFiles;
  final VoidCallback? onChat;

  @override
  Widget build(BuildContext context) => Positioned(
    left: 28,
    right: 28,
    top: 24,
    child: Card(
      elevation: 10,
      color: Colors.white.withValues(alpha: 0.94),
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(28)),
      child: Padding(
        padding: const EdgeInsets.all(18),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              selectedRoom == null
                  ? '관리 폴더를 먼저 연결해 주세요.'
                  : '${selectedRoom!['name'] as String? ?? '관리 폴더'}에서 무엇을 볼까요?',
              style: Theme.of(
                context,
              ).textTheme.titleMedium?.copyWith(fontWeight: FontWeight.w800),
            ),
            const SizedBox(height: 12),
            Row(
              children: [
                Expanded(
                  child: FilledButton.icon(
                    onPressed: onFiles,
                    icon: const Icon(Icons.folder_open),
                    label: const Text('파일 목록'),
                  ),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: FilledButton.tonalIcon(
                    onPressed: onChat,
                    icon: const Icon(Icons.chat_bubble_outline),
                    label: const Text('대화'),
                  ),
                ),
              ],
            ),
          ],
        ),
      ),
    ),
  );
}

class _HomeActionGrid extends StatelessWidget {
  const _HomeActionGrid({
    required this.selectedRoom,
    required this.onFiles,
    required this.onChat,
    required this.onHistory,
  });

  final Map<String, dynamic>? selectedRoom;
  final VoidCallback? onFiles;
  final VoidCallback? onChat;
  final VoidCallback? onHistory;

  @override
  Widget build(BuildContext context) => Wrap(
    spacing: 10,
    runSpacing: 10,
    children: [
      _HomeActionButton(
        icon: Icons.folder_open,
        label: '파일 목록',
        onPressed: onFiles,
      ),
      _HomeActionButton(
        icon: Icons.chat_bubble_outline,
        label: 'AI 대화',
        onPressed: onChat,
      ),
      _HomeActionButton(
        icon: Icons.history,
        label: '히스토리',
        onPressed: onHistory,
      ),
    ],
  );
}

class _HomeActionButton extends StatelessWidget {
  const _HomeActionButton({
    required this.icon,
    required this.label,
    required this.onPressed,
  });

  final IconData icon;
  final String label;
  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) => SizedBox(
    width: 112,
    child: FilledButton.tonalIcon(
      onPressed: onPressed,
      icon: Icon(icon, size: 18),
      label: Text(label),
    ),
  );
}

class _ConnectionSummary extends StatelessWidget {
  const _ConnectionSummary({required this.devices, required this.rooms});

  final List<Map<String, dynamic>> devices;
  final List<Map<String, dynamic>> rooms;

  @override
  Widget build(BuildContext context) {
    final onlineDevices = devices
        .where(
          (device) =>
              (device['presence'] as String? ?? '').startsWith('ONLINE'),
        )
        .length;
    final pending = rooms.fold<int>(
      0,
      (sum, room) => sum + (room['pendingProposalCount'] as int? ?? 0),
    );
    return Card(
      color: Colors.white.withValues(alpha: 0.88),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Row(
          children: [
            Expanded(
              child: _SummaryMetric(
                label: '연결된 PC',
                value: '$onlineDevices/${devices.length}',
              ),
            ),
            Expanded(
              child: _SummaryMetric(label: '관리 폴더', value: '${rooms.length}'),
            ),
            Expanded(
              child: _SummaryMetric(label: '승인 대기', value: '$pending'),
            ),
          ],
        ),
      ),
    );
  }
}

class _SummaryMetric extends StatelessWidget {
  const _SummaryMetric({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) => Column(
    children: [
      Text(
        value,
        style: Theme.of(
          context,
        ).textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w900),
      ),
      const SizedBox(height: 2),
      Text(label, style: Theme.of(context).textTheme.labelMedium),
    ],
  );
}

class _OutboxNotice extends StatelessWidget {
  const _OutboxNotice({
    required this.pending,
    required this.failed,
    required this.onDiscardFailed,
  });

  final int pending;
  final int failed;
  final VoidCallback onDiscardFailed;

  @override
  Widget build(BuildContext context) => Card(
    color: failed > 0 ? const Color(0xFFFFEBEE) : const Color(0xFFE3F2FD),
    child: ListTile(
      leading: Icon(failed > 0 ? Icons.error_outline : Icons.outbox_outlined),
      title: Text(failed > 0 ? '전송하지 못한 요청 $failed건' : '연결 후 전송할 요청 $pending건'),
      subtitle: Text(
        failed > 0
            ? '서버가 거절한 요청입니다. 상태를 확인한 뒤 정리해 주세요.'
            : '같은 idempotency key로 안전하게 다시 전송됩니다.',
      ),
      trailing: failed > 0
          ? TextButton(onPressed: onDiscardFailed, child: const Text('정리'))
          : null,
    ),
  );
}

class PushNotificationStatusCard extends StatelessWidget {
  const PushNotificationStatusCard({super.key, required this.state});

  final AsyncValue<PushNotificationRegistration> state;

  @override
  Widget build(BuildContext context) => state.when(
    loading: () => const Card(
      child: ListTile(
        leading: SizedBox.square(
          dimension: 20,
          child: CircularProgressIndicator(strokeWidth: 2),
        ),
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
      child: Card(
        color: Colors.white.withValues(alpha: 0.92),
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Icon(Icons.cloud_off_outlined, size: 48),
              const SizedBox(height: 12),
              Text('서버와 연결하지 못했습니다.\n$error', textAlign: TextAlign.center),
              const SizedBox(height: 12),
              FilledButton(onPressed: onRetry, child: const Text('다시 시도')),
            ],
          ),
        ),
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
      subtitle: Text('마지막으로 동기화된 상태만 보여줍니다.'),
    ),
  );
}

class EmptyRoomsCard extends StatelessWidget {
  const EmptyRoomsCard({super.key});

  @override
  Widget build(BuildContext context) => const Card(
    child: ListTile(
      leading: Icon(Icons.meeting_room_outlined),
      title: Text('연결된 관리 폴더가 없습니다'),
      subtitle: Text('PC에서 관리 폴더를 등록하면 여기에 표시됩니다.'),
    ),
  );
}
