import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:mousekeeper_character_assets/character_assets.dart';

import '../../core/notifications/push_notifications.dart';
import '../../core/sync/realtime_controller.dart';
import '../auth/auth_controller.dart';
import '../auth/connection_gate_controller.dart';
import '../chat/chat_page.dart';
import '../files/files_page.dart';
import '../settings/settings_page.dart';
import 'home_controller.dart';

const maxManagedFolderCount = 5;
const _mouseMoveDuration = Duration(milliseconds: 720);
const _mouseMoveCurve = Curves.easeInOutCubic;
const _mouseDisplaySize = 263.0;
const _mouseFixedYAlignment = 0.22;

enum _SpeechBubbleStage { hidden, ellipsis, menu }

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
  final _random = math.Random();
  Offset _mouseAlignment = const Offset(-0.08, _mouseFixedYAlignment);
  _SpeechBubbleStage _bubbleStage = _SpeechBubbleStage.hidden;
  String? _selectedRoomId;
  bool _mouseWalking = false;
  bool _mouseMovingRight = false;
  Timer? _walkTimer;
  Timer? _randomWalkTimer;

  @override
  void initState() {
    super.initState();
    unawaited(ref.read(realtimeRevisionProvider.notifier).connect());
    _reconcileLoop = HomeAuthoritativeReconcileLoop(
      reconcile: () =>
          ref.read(connectionGateControllerProvider.notifier).reconcile(),
    )..start();
    _scheduleRandomWalk();
  }

  @override
  void dispose() {
    _walkTimer?.cancel();
    _randomWalkTimer?.cancel();
    _reconcileLoop.dispose();
    ref.read(realtimeRevisionProvider.notifier).disconnect();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(homeControllerProvider);
    final pushNotifications = ref.watch(pushNotificationsProvider);
    final gateData = ref.watch(connectionGateControllerProvider).asData?.value;
    return Scaffold(
      extendBodyBehindAppBar: true,
      appBar: AppBar(
        backgroundColor: Colors.transparent,
        centerTitle: true,
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
        loading: () => const _HomeStage(
          backgroundAsset: null,
          mouseAlignment: Offset.zero,
          child: Center(child: CircularProgressIndicator()),
        ),
        error: (error, _) => _HomeStage(
          backgroundAsset: null,
          mouseAlignment: Offset.zero,
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
          ).take(maxManagedFolderCount).toList(growable: false);
          final selectedRoom = _selectedRoom(rooms);
          final selectedRoomIndex = selectedRoom == null
              ? 0
              : rooms.indexWhere((room) => room['id'] == selectedRoom['id']);
          final backgroundAsset = mousekeeperHomeBackgroundAssetForIndex(
            selectedRoomIndex < 0 ? 0 : selectedRoomIndex,
          );
          return _HomeStage(
            backgroundAsset: backgroundAsset,
            mouseAlignment: _mouseAlignment,
            dimBackground: _bubbleStage != _SpeechBubbleStage.hidden,
            onTapStage: _handleStageTap,
            child: LayoutBuilder(
              builder: (context, constraints) => Stack(
                children: [
                  Positioned(
                    top: MediaQuery.paddingOf(context).top + kToolbarHeight + 8,
                    left: constraints.maxWidth * 0.05,
                    right: constraints.maxWidth * 0.05,
                    child: Align(
                      alignment: Alignment.topLeft,
                      child: _ManagedFolderSelector(
                        rooms: rooms,
                        selectedRoomId: selectedRoom?['id'] as String?,
                        hiddenRoomCount:
                            ((gateData?.rooms.length ?? rooms.length) -
                                    rooms.length)
                                .clamp(0, 999),
                        onChanged: (roomId) {
                          setState(() {
                            _selectedRoomId = roomId;
                            _bubbleStage = _SpeechBubbleStage.hidden;
                          });
                        },
                      ),
                    ),
                  ),
                  Positioned(
                    top:
                        MediaQuery.paddingOf(context).top + kToolbarHeight + 86,
                    left: 16,
                    right: 16,
                    child: Column(
                      children: [
                        PushNotificationStatusCard(state: pushNotifications),
                        if (data.isOffline) const OfflineCacheBanner(),
                        if (data.outboxPending > 0 || data.outboxFailed > 0)
                          _OutboxNotice(
                            pending: data.outboxPending,
                            failed: data.outboxFailed,
                            onDiscardFailed: () => ref
                                .read(homeControllerProvider.notifier)
                                .discardFailedMutations(),
                          ),
                      ],
                    ),
                  ),
                  Positioned.fill(
                    child: _MousePlayground(
                      mouseAlignment: _mouseAlignment,
                      mouseAsset: _mouseAsset,
                      mouseWalking: _mouseWalking,
                      mouseMovingRight: _mouseMovingRight,
                      bubbleStage: _bubbleStage,
                      selectedRoom: selectedRoom,
                      onMouseTap: _handleMouseTap,
                      onBubbleTap: _handleBubbleTap,
                      onFiles: selectedRoom == null
                          ? null
                          : () => _openFiles(selectedRoom),
                      onChat: selectedRoom == null
                          ? null
                          : () => _openChat(selectedRoom),
                      onSettings: () =>
                          _openSettings(devices: devices, rooms: rooms),
                    ),
                  ),
                  Align(
                    alignment: const Alignment(0, 0.90),
                    child: _DummyBottomMenu(),
                  ),
                  if (devices.isEmpty && rooms.isEmpty)
                    const Positioned(
                      left: 18,
                      right: 18,
                      bottom: 108,
                      child: EmptyRoomsCard(),
                    ),
                ],
              ),
            ),
          );
        },
      ),
    );
  }

  void _handleStageTap(TapUpDetails details, Size size) {
    final dx = (details.localPosition.dx / size.width) * 2 - 1;
    _startMouseWalk(
      Offset(dx.clamp(-0.72, 0.72).toDouble(), _mouseFixedYAlignment),
      hideBubble: true,
    );
  }

  void _startMouseWalk(Offset target, {bool hideBubble = false}) {
    _walkTimer?.cancel();
    final movingRight = target.dx > _mouseAlignment.dx;
    setState(() {
      if (hideBubble) _bubbleStage = _SpeechBubbleStage.hidden;
      _mouseMovingRight = movingRight;
      _mouseWalking = true;
      _mouseAlignment = target;
    });
    _walkTimer = Timer(_mouseMoveDuration, () {
      if (!mounted) return;
      setState(() {
        _mouseWalking = false;
      });
    });
  }

  void _handleMouseTap() {
    _walkTimer?.cancel();
    setState(() {
      _mouseWalking = false;
      _bubbleStage = _bubbleStage == _SpeechBubbleStage.hidden
          ? _SpeechBubbleStage.ellipsis
          : _SpeechBubbleStage.hidden;
    });
  }

  void _handleBubbleTap() {
    setState(() {
      _bubbleStage = _bubbleStage == _SpeechBubbleStage.ellipsis
          ? _SpeechBubbleStage.menu
          : _SpeechBubbleStage.ellipsis;
    });
  }

  String get _mouseAsset {
    if (_mouseWalking) return mousekeeperMouseWalkGif;
    if (_bubbleStage != _SpeechBubbleStage.hidden) {
      return mousekeeperMouseWorkGif;
    }
    return mousekeeperMouseIdleGif;
  }

  void _scheduleRandomWalk() {
    _randomWalkTimer?.cancel();
    final delay = Duration(seconds: 4 + _random.nextInt(5));
    _randomWalkTimer = Timer(delay, () {
      if (!mounted) return;
      if (!_mouseWalking && _bubbleStage == _SpeechBubbleStage.hidden) {
        _startMouseWalk(_nextRandomMouseTarget());
      }
      _scheduleRandomWalk();
    });
  }

  Offset _nextRandomMouseTarget() {
    final distance = 0.22 + _random.nextDouble() * 0.36;
    final shouldMoveRight =
        _mouseAlignment.dx < -0.58 || _random.nextDouble() < 0.28;
    final nextDx =
        (_mouseAlignment.dx + (shouldMoveRight ? distance : -distance))
            .clamp(-0.72, 0.72)
            .toDouble();
    return Offset(nextDx, _mouseFixedYAlignment);
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

  void _openSettings({
    required List<Map<String, dynamic>> devices,
    required List<Map<String, dynamic>> rooms,
  }) {
    Navigator.of(context).push(
      MaterialPageRoute(
        builder: (_) => MouseKeeperSettingsPage(devices: devices, rooms: rooms),
      ),
    );
  }
}

class _HomeStage extends StatelessWidget {
  const _HomeStage({
    required this.child,
    required this.backgroundAsset,
    required this.mouseAlignment,
    this.onTapStage,
    this.dimBackground = false,
  });

  final Widget child;
  final String? backgroundAsset;
  final Offset mouseAlignment;
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
            _RoomPanningBackground(
              asset: backgroundAsset,
              mouseAlignment: mouseAlignment,
            ),
            AnimatedContainer(
              duration: const Duration(milliseconds: 160),
              color: dimBackground
                  ? Colors.black.withValues(alpha: 0.18)
                  : Colors.transparent,
            ),
            child,
          ],
        ),
      );
    },
  );
}

class _RoomPanningBackground extends StatelessWidget {
  const _RoomPanningBackground({
    required this.asset,
    required this.mouseAlignment,
  });

  final String? asset;
  final Offset mouseAlignment;

  @override
  Widget build(BuildContext context) {
    final background = asset;
    if (background == null || background.isEmpty) {
      return const DecoratedBox(
        decoration: BoxDecoration(
          gradient: LinearGradient(
            begin: Alignment.topCenter,
            end: Alignment.bottomCenter,
            colors: [Color(0xFFFFF6E9), Color(0xFFE8F4FF)],
          ),
        ),
      );
    }
    return LayoutBuilder(
      builder: (context, constraints) {
        final imageWidth = constraints.maxWidth * 1.28;
        final extraWidth = imageWidth - constraints.maxWidth;
        final progress = ((mouseAlignment.dx + 1) / 2).clamp(0.0, 1.0);
        final left = -extraWidth * progress;
        return Stack(
          fit: StackFit.expand,
          children: [
            AnimatedPositioned(
              duration: _mouseMoveDuration,
              curve: _mouseMoveCurve,
              left: left,
              top: 0,
              width: imageWidth,
              height: constraints.maxHeight,
              child: Image.asset(
                background,
                package: mousekeeperMascotPackage,
                fit: BoxFit.cover,
                filterQuality: FilterQuality.low,
              ),
            ),
            DecoratedBox(
              decoration: BoxDecoration(
                gradient: LinearGradient(
                  begin: Alignment.topCenter,
                  end: Alignment.bottomCenter,
                  colors: [
                    Colors.white.withValues(alpha: 0.16),
                    const Color(0xFFFFF2E5).withValues(alpha: 0.28),
                  ],
                ),
              ),
            ),
          ],
        );
      },
    );
  }
}

class _ManagedFolderSelector extends StatelessWidget {
  const _ManagedFolderSelector({
    required this.rooms,
    required this.selectedRoomId,
    required this.hiddenRoomCount,
    required this.onChanged,
  });

  final List<Map<String, dynamic>> rooms;
  final String? selectedRoomId;
  final int hiddenRoomCount;
  final ValueChanged<String> onChanged;

  @override
  Widget build(BuildContext context) => DecoratedBox(
    decoration: BoxDecoration(
      color: Colors.white.withValues(alpha: 0.90),
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
                  for (var index = 0; index < rooms.length; index++)
                    DropdownMenuItem<String>(
                      value: rooms[index]['id'] as String?,
                      child: Text(
                        '${index + 1}. ${rooms[index]['name'] as String? ?? '관리 폴더'}',
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                ],
                onChanged: (value) {
                  if (value != null) onChanged(value);
                },
              ),
            ),
          if (hiddenRoomCount > 0) ...[
            const SizedBox(width: 6),
            Tooltip(
              message: '앱 홈에서는 최대 5개 폴더만 표시합니다.',
              child: Chip(
                visualDensity: VisualDensity.compact,
                label: Text('+$hiddenRoomCount'),
              ),
            ),
          ],
        ],
      ),
    ),
  );
}

class _MousePlayground extends StatelessWidget {
  const _MousePlayground({
    required this.mouseAlignment,
    required this.mouseAsset,
    required this.mouseWalking,
    required this.mouseMovingRight,
    required this.bubbleStage,
    required this.selectedRoom,
    required this.onMouseTap,
    required this.onBubbleTap,
    required this.onFiles,
    required this.onChat,
    required this.onSettings,
  });

  final Offset mouseAlignment;
  final String mouseAsset;
  final bool mouseWalking;
  final bool mouseMovingRight;
  final _SpeechBubbleStage bubbleStage;
  final Map<String, dynamic>? selectedRoom;
  final VoidCallback onMouseTap;
  final VoidCallback onBubbleTap;
  final VoidCallback? onFiles;
  final VoidCallback? onChat;
  final VoidCallback onSettings;

  @override
  Widget build(BuildContext context) => Stack(
    clipBehavior: Clip.none,
    children: [
      if (bubbleStage != _SpeechBubbleStage.hidden)
        _MouseSpeechBubble(
          alignment: Offset(mouseAlignment.dx, mouseAlignment.dy + 0.58),
          stage: bubbleStage,
          selectedRoom: selectedRoom,
          onTap: onBubbleTap,
          onFiles: onFiles,
          onChat: onChat,
          onSettings: onSettings,
        ),
      AnimatedAlign(
        duration: _mouseMoveDuration,
        curve: _mouseMoveCurve,
        alignment: Alignment(mouseAlignment.dx, mouseAlignment.dy),
        child: GestureDetector(
          behavior: HitTestBehavior.translucent,
          onTap: onMouseTap,
          child: Transform(
            alignment: Alignment.center,
            transform: Matrix4.diagonal3Values(
              mouseWalking && mouseMovingRight ? -1.0 : 1.0,
              1.0,
              1.0,
            ),
            child: Image.asset(
              mouseAsset,
              key: ValueKey('$mouseAsset-$mouseMovingRight'),
              package: mousekeeperMascotPackage,
              width: _mouseDisplaySize,
              height: _mouseDisplaySize,
              fit: BoxFit.contain,
              filterQuality: FilterQuality.none,
            ),
          ),
        ),
      ),
    ],
  );
}

class _MouseSpeechBubble extends StatelessWidget {
  const _MouseSpeechBubble({
    required this.alignment,
    required this.stage,
    required this.selectedRoom,
    required this.onTap,
    required this.onFiles,
    required this.onChat,
    required this.onSettings,
  });

  final Offset alignment;
  final _SpeechBubbleStage stage;
  final Map<String, dynamic>? selectedRoom;
  final VoidCallback onTap;
  final VoidCallback? onFiles;
  final VoidCallback? onChat;
  final VoidCallback onSettings;

  @override
  Widget build(BuildContext context) => AnimatedAlign(
    duration: _mouseMoveDuration,
    curve: _mouseMoveCurve,
    alignment: Alignment(
      alignment.dx.clamp(-0.72, 0.72),
      alignment.dy.clamp(-0.58, 0.82),
    ),
    child: GestureDetector(
      onTap: onTap,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: const Color(0xFFDFF4FF).withValues(alpha: 0.96),
          borderRadius: BorderRadius.circular(28),
          border: Border.all(color: const Color(0xFF8AC7E8)),
          boxShadow: [
            BoxShadow(
              color: Colors.black.withValues(alpha: 0.12),
              blurRadius: 16,
              offset: const Offset(0, 8),
            ),
          ],
        ),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 14),
          child: stage == _SpeechBubbleStage.ellipsis
              ? Text(
                  '…',
                  style: Theme.of(context).textTheme.headlineMedium?.copyWith(
                    color: const Color(0xFF436577),
                    fontWeight: FontWeight.w900,
                  ),
                )
              : Wrap(
                  alignment: WrapAlignment.center,
                  runSpacing: 8,
                  spacing: 10,
                  children: [
                    FilledButton.icon(
                      onPressed: onFiles,
                      icon: const Icon(Icons.folder_open),
                      label: const Text('파일 목록'),
                    ),
                    FilledButton.tonalIcon(
                      onPressed: onChat,
                      icon: const Icon(Icons.chat_bubble_outline),
                      label: const Text('대화'),
                    ),
                    FilledButton.tonalIcon(
                      onPressed: onSettings,
                      icon: const Icon(Icons.settings_outlined),
                      label: const Text('설정'),
                    ),
                  ],
                ),
        ),
      ),
    ),
  );
}

class _DummyBottomMenu extends StatelessWidget {
  @override
  Widget build(BuildContext context) => DecoratedBox(
    decoration: BoxDecoration(
      color: Colors.white.withValues(alpha: 0.90),
      borderRadius: BorderRadius.circular(28),
      boxShadow: [
        BoxShadow(
          color: Colors.black.withValues(alpha: 0.08),
          blurRadius: 16,
          offset: const Offset(0, 8),
        ),
      ],
    ),
    child: Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      child: Row(
        children: const [
          Expanded(
            child: _DummyBottomMenuButton(icon: Icons.extension, label: '퍼즐'),
          ),
          Expanded(
            child: _DummyBottomMenuButton(icon: Icons.restaurant, label: '식사'),
          ),
          Expanded(
            child: _DummyBottomMenuButton(
              icon: Icons.sports_esports,
              label: '게임',
            ),
          ),
        ],
      ),
    ),
  );
}

class _DummyBottomMenuButton extends StatelessWidget {
  const _DummyBottomMenuButton({required this.icon, required this.label});

  final IconData icon;
  final String label;

  @override
  Widget build(BuildContext context) => Opacity(
    opacity: 0.62,
    child: Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(icon),
        const SizedBox(height: 4),
        Text(label, style: Theme.of(context).textTheme.labelMedium),
      ],
    ),
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
