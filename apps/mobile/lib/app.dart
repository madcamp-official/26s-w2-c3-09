import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'features/auth/auth_controller.dart';
import 'features/auth/login_page.dart';
import 'features/home/home_page.dart';
import 'core/sync/realtime_controller.dart';

final mousekeeperScaffoldMessengerKey = GlobalKey<ScaffoldMessengerState>();

class MouseKeeperApp extends ConsumerWidget {
  const MouseKeeperApp({super.key, this.configurationError});
  final String? configurationError;
  @override
  Widget build(BuildContext context, WidgetRef ref) {
    ref.listen(realtimeNoticeProvider, (previous, next) {
      if (next == null || previous?.eventId == next.eventId) return;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        final messenger = mousekeeperScaffoldMessengerKey.currentState;
        messenger
          ?..hideCurrentSnackBar()
          ..showSnackBar(SnackBar(content: Text(next.message)));
      });
    });
    return MaterialApp(
      title: 'MOUSEKEEPER',
      scaffoldMessengerKey: mousekeeperScaffoldMessengerKey,
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: const Color(0xFF76543A)),
        useMaterial3: true,
      ),
      home: configurationError != null
          ? ConfigurationRequiredPage(error: configurationError!)
          : ref
                .watch(authControllerProvider)
                .when(
                  data: (user) =>
                      user == null ? const LoginPage() : const HomePage(),
                  loading: () => const Scaffold(
                    body: Center(child: CircularProgressIndicator()),
                  ),
                  error: (error, _) =>
                      Scaffold(body: Center(child: Text('인증 상태 오류: $error'))),
                ),
    );
  }
}

class ConfigurationRequiredPage extends StatelessWidget {
  const ConfigurationRequiredPage({super.key, required this.error});
  final String error;
  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: AppBar(title: const Text('MOUSEKEEPER')),
    body: Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Icon(Icons.settings_outlined, size: 56),
            const SizedBox(height: 20),
            const Text('Google 로그인이 아직 설정되지 않았습니다.'),
            const SizedBox(height: 8),
            Text(
              '설정 누락: $error\n오류 코드: UNCONFIGURED',
              textAlign: TextAlign.center,
            ),
          ],
        ),
      ),
    ),
  );
}
