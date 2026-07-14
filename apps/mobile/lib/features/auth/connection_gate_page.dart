import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:mousekeeper_character_assets/character_assets.dart';

import '../home/home_page.dart';
import 'auth_controller.dart';
import 'connection_gate_controller.dart';
import 'pairing_page.dart';

class ConnectionGatePage extends ConsumerWidget {
  const ConnectionGatePage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final gate = ref.watch(connectionGateControllerProvider);
    return gate.when(
      loading: () => DeferredPairingGateLoadingPage(
        onRetry: () =>
            ref.read(connectionGateControllerProvider.notifier).retryLoad(),
      ),
      error: (error, _) => PairingGateErrorPage(
        error: error,
        onRetry: () =>
            ref.read(connectionGateControllerProvider.notifier).retryLoad(),
        onSignOut: () => ref.read(authControllerProvider.notifier).signOut(),
      ),
      data: (data) => data.hasActiveDevice
          ? const MainNavigationHost(key: ValueKey('paired-main'))
          : PairingPage(
              key: const ValueKey('pairing-only'),
              gateMode: true,
              onClaim: ref
                  .read(connectionGateControllerProvider.notifier)
                  .claimAndConfirm,
            ),
    );
  }
}

enum PairingGateLoadingStage {
  authenticating(0.15, '로그인 상태를 확인하는 중입니다'),
  loadingConnections(0.35, '연결된 PC를 확인하는 중입니다'),
  connectingRealtime(0.60, '실시간 연결을 준비하는 중입니다'),
  reconcilingCache(0.80, '표시 상태를 맞추는 중입니다'),
  ready(1.0, '연결 준비가 끝났습니다');

  const PairingGateLoadingStage(this.progress, this.message);

  final double progress;
  final String message;
}

class PairingGateLoadingPage extends StatelessWidget {
  const PairingGateLoadingPage({
    super.key,
    this.stage = PairingGateLoadingStage.loadingConnections,
    this.showLongWaitMessage = false,
    this.onRetry,
  });

  final PairingGateLoadingStage stage;
  final bool showLongWaitMessage;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) {
    final disableAnimations =
        MediaQuery.maybeOf(context)?.disableAnimations ?? false;
    return Scaffold(
      body: Center(
        child: Padding(
          padding: const EdgeInsets.all(28),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              PixelFillMouse(
                progress: stage.progress,
                animate: !disableAnimations,
              ),
              const SizedBox(height: 20),
              Text(stage.message, textAlign: TextAlign.center),
              const SizedBox(height: 8),
              Text(
                '${(stage.progress * 100).round()}%',
                style: Theme.of(context).textTheme.labelMedium,
              ),
              if (showLongWaitMessage) ...[
                const SizedBox(height: 16),
                const Text(
                  '연결 확인이 예상보다 오래 걸리고 있어요.',
                  textAlign: TextAlign.center,
                ),
              ],
              if (onRetry != null) ...[
                const SizedBox(height: 12),
                OutlinedButton.icon(
                  onPressed: onRetry,
                  icon: const Icon(Icons.refresh),
                  label: const Text('다시 확인'),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

class DeferredPairingGateLoadingPage extends StatefulWidget {
  const DeferredPairingGateLoadingPage({
    super.key,
    required this.onRetry,
    this.showAfter = const Duration(milliseconds: 200),
    this.longWaitAfter = const Duration(seconds: 10),
    this.retryAfter = const Duration(seconds: 20),
  });

  final VoidCallback onRetry;
  final Duration showAfter;
  final Duration longWaitAfter;
  final Duration retryAfter;

  @override
  State<DeferredPairingGateLoadingPage> createState() =>
      _DeferredPairingGateLoadingPageState();
}

class _DeferredPairingGateLoadingPageState
    extends State<DeferredPairingGateLoadingPage> {
  final List<Timer> _timers = [];
  bool _visible = false;
  bool _longWait = false;
  bool _showRetry = false;

  @override
  void initState() {
    super.initState();
    _schedule();
  }

  @override
  void dispose() {
    for (final timer in _timers) {
      timer.cancel();
    }
    _timers.clear();
    super.dispose();
  }

  void _schedule() {
    _after(widget.showAfter, () => _visible = true);
    _after(widget.longWaitAfter, () => _longWait = true);
    _after(widget.retryAfter, () => _showRetry = true);
  }

  void _after(Duration duration, void Function() update) {
    if (duration <= Duration.zero) {
      update();
      return;
    }
    _timers.add(
      Timer(duration, () {
        if (!mounted) return;
        setState(update);
      }),
    );
  }

  @override
  Widget build(BuildContext context) {
    if (!_visible) return const Scaffold(body: SizedBox.shrink());
    return PairingGateLoadingPage(
      showLongWaitMessage: _longWait,
      onRetry: _showRetry ? widget.onRetry : null,
    );
  }
}

class PixelFillMouse extends StatelessWidget {
  const PixelFillMouse({
    super.key,
    required this.progress,
    this.size = 128,
    this.animate = true,
  });

  final double progress;
  final double size;
  final bool animate;

  @override
  Widget build(BuildContext context) {
    final clamped = progress.clamp(0, 1).toDouble();
    if (!animate) {
      return _PixelFillMouseFrame(progress: clamped, size: size);
    }
    return TweenAnimationBuilder<double>(
      duration: const Duration(milliseconds: 700),
      curve: Curves.easeOutCubic,
      tween: Tween<double>(begin: 0, end: clamped),
      builder: (context, value, _) =>
          _PixelFillMouseFrame(progress: value, size: size),
    );
  }
}

class _PixelFillMouseFrame extends StatelessWidget {
  const _PixelFillMouseFrame({required this.progress, required this.size});

  final double progress;
  final double size;

  @override
  Widget build(BuildContext context) => Semantics(
    label: 'MouseKeeper loading ${(progress * 100).round()} percent',
    child: SizedBox.square(
      dimension: size,
      child: Stack(
        alignment: Alignment.bottomCenter,
        children: [
          Opacity(opacity: 0.20, child: _mouseImage()),
          ClipRect(
            child: Align(
              alignment: Alignment.bottomCenter,
              heightFactor: progress,
              child: _mouseImage(),
            ),
          ),
        ],
      ),
    ),
  );

  Widget _mouseImage() => Image.asset(
    mousekeeperStandAsset,
    package: mousekeeperMascotPackage,
    width: size,
    height: size,
    fit: BoxFit.contain,
    filterQuality: FilterQuality.none,
  );
}

class PairingGateErrorPage extends StatelessWidget {
  const PairingGateErrorPage({
    super.key,
    required this.error,
    required this.onRetry,
    required this.onSignOut,
  });

  final Object error;
  final VoidCallback onRetry;
  final VoidCallback onSignOut;

  @override
  Widget build(BuildContext context) => Scaffold(
    body: Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Icon(Icons.cloud_off_outlined, size: 56),
            const SizedBox(height: 16),
            const Text(
              '서버에서 활성 PC 상태를 확인하지 못했습니다.\n'
              '페어링이 끝나기 전에는 메인 화면으로 이동하지 않습니다.',
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 8),
            Text('$error', textAlign: TextAlign.center),
            const SizedBox(height: 16),
            FilledButton.icon(
              onPressed: onRetry,
              icon: const Icon(Icons.refresh),
              label: const Text('다시 확인'),
            ),
            const SizedBox(height: 8),
            TextButton(onPressed: onSignOut, child: const Text('로그아웃')),
          ],
        ),
      ),
    ),
  );
}

class MainNavigationHost extends StatefulWidget {
  const MainNavigationHost({super.key});

  @override
  State<MainNavigationHost> createState() => _MainNavigationHostState();
}

class _MainNavigationHostState extends State<MainNavigationHost> {
  final _navigatorKey = GlobalKey<NavigatorState>();

  @override
  Widget build(BuildContext context) => Navigator(
    key: _navigatorKey,
    onGenerateRoute: (settings) => MaterialPageRoute<void>(
      settings: settings,
      builder: (_) => const HomePage(),
    ),
  );
}
