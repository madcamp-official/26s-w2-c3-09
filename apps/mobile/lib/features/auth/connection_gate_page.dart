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
      loading: () => const PairingGateLoadingPage(),
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
  reconcilingCache(0.80, '로컬 표시 상태를 맞추는 중입니다'),
  ready(1.0, '연결 준비가 끝났습니다');

  const PairingGateLoadingStage(this.progress, this.message);

  final double progress;
  final String message;
}

class PairingGateLoadingPage extends StatelessWidget {
  const PairingGateLoadingPage({
    super.key,
    this.stage = PairingGateLoadingStage.loadingConnections,
  });

  final PairingGateLoadingStage stage;

  @override
  Widget build(BuildContext context) {
    final disableAnimations =
        MediaQuery.maybeOf(context)?.disableAnimations ?? false;
    return Scaffold(
      body: Center(
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
          ],
        ),
      ),
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
    appBar: AppBar(
      title: const Text('PC 연결 확인'),
      actions: [
        IconButton(
          tooltip: '로그아웃',
          onPressed: onSignOut,
          icon: const Icon(Icons.logout),
        ),
      ],
    ),
    body: Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Icon(Icons.cloud_off_outlined, size: 56),
            const SizedBox(height: 16),
            const Text(
              '서버에서 활성 PC 상태를 확인하지 못했습니다.\n이전 캐시로 메인 화면을 열지 않습니다.',
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
