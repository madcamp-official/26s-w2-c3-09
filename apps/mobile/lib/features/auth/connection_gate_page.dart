import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

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

class PairingGateLoadingPage extends StatelessWidget {
  const PairingGateLoadingPage({super.key});

  @override
  Widget build(BuildContext context) => const Scaffold(
    body: Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          CircularProgressIndicator(),
          SizedBox(height: 16),
          Text('연결된 PC를 확인하는 중입니다'),
        ],
      ),
    ),
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
