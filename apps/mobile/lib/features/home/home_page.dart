import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:mousekeeper_character_assets/character_assets.dart';

import '../../core/notifications/push_notifications.dart';
import '../../core/sync/realtime_controller.dart';
import '../../core/widgets/cheese_loading.dart';
import '../auth/connection_gate_controller.dart';
import '../chat/chat_page.dart';
import '../feeding/feeding_interaction.dart';
import '../files/files_page.dart';
import '../games/mini_game_hub_page.dart';
import '../navigation/home_bottom_navigation.dart';
import '../settings/settings_page.dart';
import '../wardrobe/mouse_wardrobe_page.dart';
import 'home_controller.dart';

const maxManagedFolderCount = 5;
const _mouseMoveDuration = Duration(milliseconds: 1440);
const _mouseMoveCurve = Curves.easeInOutCubic;
const _mouseDisplaySize = 263.0;
const _mouseFixedYAlignment = 0.22;

const _pixelInk = Color(0xFF3A2A1F);
const _pixelPaper = Color(0xFFFFE9B8);

class _PixelCard extends StatelessWidget {
  const _PixelCard({required this.child, this.color});

  final Widget child;
  final Color? color;

  @override
  Widget build(BuildContext context) => Card(
    color: color ?? _pixelPaper,
    elevation: 0,
    margin: const EdgeInsets.only(right: 5, bottom: 5),
    shape: const BeveledRectangleBorder(
      side: BorderSide(color: _pixelInk, width: 2),
      borderRadius: BorderRadius.all(Radius.circular(3)),
    ),
    shadowColor: Colors.transparent,
    child: DecoratedBox(
      decoration: const BoxDecoration(
        boxShadow: [
          BoxShadow(color: _pixelInk, offset: Offset(5, 5), blurRadius: 0),
        ],
      ),
      child: child,
    ),
  );
}

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
  late final FeedingController _feedingController;
  final _random = math.Random();
  final _stageKey = GlobalKey();
  final _mouseKey = GlobalKey();
  Offset _mouseAlignment = const Offset(-0.08, _mouseFixedYAlignment);
  String? _selectedRoomId;
  HomeMenuAction? _selectedMenu;
  bool _mouseWalking = false;
  bool _mouseMovingRight = false;
  Timer? _walkTimer;
  Timer? _randomWalkTimer;

  @override
  void initState() {
    super.initState();
    _feedingController = FeedingController()..addListener(_onFeedingChanged);
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
    _feedingController
      ..removeListener(_onFeedingChanged)
      ..dispose();
    ref.read(realtimeRevisionProvider.notifier).disconnect();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(homeControllerProvider);
    final gateData = ref.watch(connectionGateControllerProvider).asData?.value;
    final menuRooms = mergeAuthoritativeConnectionItems(
      authoritative: gateData?.rooms ?? const [],
      enriched: state.asData?.value.rooms ?? const [],
    ).take(maxManagedFolderCount).toList(growable: false);
    final menuSelectedRoom = _selectedRoom(menuRooms);
    final horizontalInset = MediaQuery.sizeOf(context).width * 0.05;
    return Scaffold(
      extendBodyBehindAppBar: true,
      appBar: AppBar(
        backgroundColor: Colors.transparent,
        elevation: 0,
        toolbarHeight: 64,
        titleSpacing: horizontalInset,
        title: _ManagedFolderSelector(
          rooms: menuRooms,
          selectedRoomId: menuSelectedRoom?['id'] as String?,
          hiddenRoomCount:
              ((gateData?.rooms.length ?? menuRooms.length) - menuRooms.length)
                  .clamp(0, 999),
          onChanged: (roomId) => setState(() => _selectedRoomId = roomId),
        ),
        actions: [
          Padding(
            padding: EdgeInsets.only(right: horizontalInset),
            child: IconButton(
              key: const ValueKey('home-settings-button'),
              tooltip: '설정',
              onPressed: _openSettings,
              icon: const Icon(
                Icons.settings_outlined,
                color: Color(0xFF4B6482),
                size: 31,
              ),
            ),
          ),
        ],
      ),
      body: state.when(
        loading: () => const _HomeStage(
          backgroundAsset: null,
          mouseAlignment: Offset.zero,
          child: CheeseLoadingView(message: '관리 폴더를 불러오는 중입니다'),
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
          final rooms = menuRooms;
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
            onTapStage:
                _feedingController.state.isActive ||
                    _feedingController.state.isFeeding
                ? null
                : _handleStageTap,
            child: LayoutBuilder(
              builder: (context, constraints) => Stack(
                key: _stageKey,
                children: [
                  if (data.isOffline ||
                      data.outboxPending > 0 ||
                      data.outboxFailed > 0)
                    Positioned(
                      top:
                          MediaQuery.paddingOf(context).top +
                          kToolbarHeight +
                          20,
                      left: constraints.maxWidth * 0.05,
                      right: constraints.maxWidth * 0.05,
                      child: Column(
                        children: [
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
                  if (devices.isEmpty && rooms.isEmpty)
                    const Positioned(
                      left: 18,
                      right: 18,
                      bottom: 108,
                      child: EmptyRoomsCard(),
                    ),
                  Positioned.fill(
                    child: _MousePlayground(
                      mouseKey: _mouseKey,
                      mouseAlignment: _mouseAlignment,
                      mouseAsset: _mouseAsset,
                      mouseWalking: _mouseWalking,
                      mouseMovingRight: _mouseMovingRight,
                      isFeeding: _feedingController.state.isFeeding,
                      onMouseTap: () => _handleMouseTap(selectedRoom),
                    ),
                  ),
                  Positioned.fill(
                    child: FeedingGestureLayer(
                      state: _feedingController.state,
                      mouseCollider: _mouseRectInStage,
                      onPointer: _handleFeedingPointer,
                    ),
                  ),
                  Positioned(
                    left: 24,
                    right: 24,
                    bottom: constraints.maxHeight * 0.05,
                    child: HomeBottomNavigation(
                      selected: _selectedMenu,
                      onSelected: (action) =>
                          unawaited(_handleMenuAction(action, selectedRoom)),
                    ),
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
    );
  }

  void _startMouseWalk(Offset target) {
    _walkTimer?.cancel();
    final movingRight = target.dx > _mouseAlignment.dx;
    setState(() {
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

  void _handleMouseTap(Map<String, dynamic>? selectedRoom) {
    _walkTimer?.cancel();
    setState(() => _mouseWalking = false);
    if (selectedRoom == null) {
      ScaffoldMessenger.of(context)
        ..hideCurrentSnackBar()
        ..showSnackBar(const SnackBar(content: Text('폴더를 연결해 주세요!')));
      return;
    }
    _openChat(selectedRoom);
  }

  String get _mouseAsset {
    if (_mouseWalking) return mousekeeperMouseWalkGif;
    return mousekeeperMouseIdleGif;
  }

  void _scheduleRandomWalk() {
    _randomWalkTimer?.cancel();
    final delay = Duration(seconds: 4 + _random.nextInt(5));
    _randomWalkTimer = Timer(delay, () {
      if (!mounted) return;
      if (!_mouseWalking &&
          !_feedingController.state.isActive &&
          !_feedingController.state.isFeeding) {
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

  void _openChat(Map<String, dynamic> room) {
    Navigator.of(context).push(
      MaterialPageRoute(
        builder: (_) => ChatPage(
          roomId: room['id'] as String,
          roomName: room['name'] as String? ?? '관리 폴더',
        ),
      ),
    );
  }

  Future<void> _handleMenuAction(
    HomeMenuAction action,
    Map<String, dynamic>? selectedRoom,
  ) async {
    if (action == HomeMenuAction.feeding) {
      _walkTimer?.cancel();
      setState(() {
        _mouseWalking = false;
        _selectedMenu = _feedingController.state.isActive ? null : action;
      });
      if (_feedingController.state.isActive) {
        _feedingController.cancel();
      } else {
        _feedingController.activate();
      }
      return;
    }

    if (_feedingController.state.isActive) _feedingController.cancel();
    setState(() => _selectedMenu = action);
    switch (action) {
      case HomeMenuAction.files:
        if (selectedRoom == null) {
          _showMissingFolderMessage();
        } else {
          await Navigator.of(context).push(
            MaterialPageRoute<void>(
              fullscreenDialog: true,
              builder: (_) => FilesPage(
                roomId: selectedRoom['id'] as String,
                roomName: selectedRoom['name'] as String? ?? '관리 폴더',
              ),
            ),
          );
        }
      case HomeMenuAction.games:
        await Navigator.of(context).push(
          MaterialPageRoute<void>(builder: (_) => const MiniGameHubPage()),
        );
      case HomeMenuAction.wardrobe:
        await Navigator.of(context).push(
          MaterialPageRoute<void>(builder: (_) => const MouseWardrobePage()),
        );
      case HomeMenuAction.feeding:
        break;
    }
    if (mounted) setState(() => _selectedMenu = null);
  }

  void _handleFeedingPointer(Offset position) {
    unawaited(_feedingController.dragCheese(position, _mouseRectInStage()));
  }

  Rect? _mouseRectInStage() {
    final stage = _stageKey.currentContext?.findRenderObject();
    final mouse = _mouseKey.currentContext?.findRenderObject();
    if (stage is! RenderBox ||
        mouse is! RenderBox ||
        !stage.hasSize ||
        !mouse.hasSize) {
      return null;
    }
    final mouseGlobal = mouse.localToGlobal(Offset.zero);
    return stage.globalToLocal(mouseGlobal) & mouse.size;
  }

  void _onFeedingChanged() {
    if (!mounted) return;
    setState(() {
      final feeding = _feedingController.state;
      if (!feeding.isActive &&
          !feeding.isFeeding &&
          _selectedMenu == HomeMenuAction.feeding) {
        _selectedMenu = null;
      }
    });
  }

  void _showMissingFolderMessage() {
    ScaffoldMessenger.of(context)
      ..hideCurrentSnackBar()
      ..showSnackBar(const SnackBar(content: Text('폴더를 연결해 주세요!')));
  }

  void _openSettings() {
    Navigator.of(
      context,
    ).push(MaterialPageRoute(builder: (_) => const MouseKeeperSettingsPage()));
  }
}

class _HomeStage extends StatelessWidget {
  const _HomeStage({
    required this.child,
    required this.backgroundAsset,
    required this.mouseAlignment,
    this.onTapStage,
  });

  final Widget child;
  final String? backgroundAsset;
  final Offset mouseAlignment;
  final void Function(TapUpDetails details, Size size)? onTapStage;

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
          color: Color(0xFFD8C58F),
          backgroundBlendMode: BlendMode.multiply,
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
                  colors: [Color(0x22FFE9B8), Color(0x553A2A1F)],
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
  Widget build(BuildContext context) => SizedBox(
    width: double.infinity,
    height: 44,
    child: DecoratedBox(
      decoration: BoxDecoration(
        color: _pixelPaper,
        border: Border.all(color: _pixelInk, width: 2),
        boxShadow: const [BoxShadow(color: _pixelInk, offset: Offset(4, 4))],
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
        child: Row(
          children: [
            const Icon(
              Icons.folder_outlined,
              size: 21,
              color: Color(0xFF4B6482),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: rooms.isEmpty
                  ? const Text(
                      '관리 폴더 없음',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    )
                  : DropdownButtonHideUnderline(
                      child: DropdownButton<String>(
                        key: const ValueKey('managed-folder-selector'),
                        value: selectedRoomId,
                        isDense: true,
                        isExpanded: true,
                        borderRadius: BorderRadius.zero,
                        items: [
                          for (var index = 0; index < rooms.length; index++)
                            DropdownMenuItem<String>(
                              value: rooms[index]['id'] as String?,
                              child: Text(
                                rooms[index]['name'] as String? ?? '관리 폴더',
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: const TextStyle(
                                  fontWeight: FontWeight.w700,
                                ),
                              ),
                            ),
                        ],
                        onChanged: (value) {
                          if (value != null) onChanged(value);
                        },
                      ),
                    ),
            ),
            if (hiddenRoomCount > 0) ...[
              const SizedBox(width: 4),
              Text(
                '+$hiddenRoomCount',
                maxLines: 1,
                style: const TextStyle(fontWeight: FontWeight.w700),
              ),
            ],
          ],
        ),
      ),
    ),
  );
}

class _MousePlayground extends StatelessWidget {
  const _MousePlayground({
    required this.mouseKey,
    required this.mouseAlignment,
    required this.mouseAsset,
    required this.mouseWalking,
    required this.mouseMovingRight,
    required this.isFeeding,
    required this.onMouseTap,
  });

  final GlobalKey mouseKey;
  final Offset mouseAlignment;
  final String mouseAsset;
  final bool mouseWalking;
  final bool mouseMovingRight;
  final bool isFeeding;
  final VoidCallback onMouseTap;

  @override
  Widget build(BuildContext context) => Stack(
    clipBehavior: Clip.none,
    children: [
      AnimatedAlign(
        duration: _mouseMoveDuration,
        curve: _mouseMoveCurve,
        alignment: Alignment(mouseAlignment.dx, mouseAlignment.dy),
        child: GestureDetector(
          key: mouseKey,
          behavior: HitTestBehavior.translucent,
          onTap: onMouseTap,
          child: AnimatedScale(
            scale: isFeeding ? 1.12 : 1,
            duration: const Duration(milliseconds: 220),
            curve: Curves.elasticOut,
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
      ),
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
    loading: () => const _PixelCard(
      child: ListTile(
        leading: SizedBox.square(
          dimension: 20,
          child: CheeseLoadingIndicator(size: 20),
        ),
        title: Text('알림 연결 중'),
      ),
    ),
    error: (error, _) => _PixelCard(
      color: const Color(0xFFFFEBEE),
      child: ListTile(
        leading: const Icon(Icons.notifications_off_outlined),
        title: const Text('푸시 알림을 연결하지 못했어요'),
        subtitle: Text('$error'),
      ),
    ),
    data: (registration) => switch (registration.status) {
      PushNotificationStatus.active => const _PixelCard(
        child: ListTile(
          leading: Icon(Icons.notifications_active_outlined),
          title: Text('알림이 연결되어 있어요'),
          subtitle: Text('작업 제안과 실행 결과를 이 휴대폰에서 알려드립니다.'),
        ),
      ),
      PushNotificationStatus.permissionDenied => const _PixelCard(
        color: Color(0xFFFFF3E0),
        child: ListTile(
          leading: Icon(Icons.notifications_off_outlined),
          title: Text('알림 권한이 꺼져 있어요'),
          subtitle: Text('휴대폰 설정에서 MOUSEKEEPER 알림을 허용해 주세요.'),
        ),
      ),
      PushNotificationStatus.unconfigured => _PixelCard(
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
      child: _PixelCard(
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
  Widget build(BuildContext context) => const _PixelCard(
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
  Widget build(BuildContext context) => const _PixelCard(
    child: ListTile(
      leading: Icon(Icons.meeting_room_outlined),
      title: Text('연결된 관리 폴더가 없습니다'),
      subtitle: Text('PC에서 관리 폴더를 등록하면 여기에 표시됩니다.'),
    ),
  );
}
