import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'features/auth/auth_controller.dart';
import 'features/auth/connection_gate_page.dart';
import 'features/auth/login_page.dart';
import 'core/sync/realtime_controller.dart';
import 'core/theme/pixel_theme.dart';
import 'core/widgets/cheese_loading.dart';

final mousekeeperScaffoldMessengerKey = GlobalKey<ScaffoldMessengerState>();

class MouseKeeperApp extends ConsumerWidget {
  const MouseKeeperApp({super.key, this.configurationError});
  final String? configurationError;
  @override
  Widget build(BuildContext context, WidgetRef ref) {
    if (configurationError == null) {
      ref.listen(realtimeNoticeProvider, (previous, next) {
        if (next == null || previous?.eventId == next.eventId) return;
        WidgetsBinding.instance.addPostFrameCallback((_) {
          final messenger = mousekeeperScaffoldMessengerKey.currentState;
          messenger
            ?..hideCurrentSnackBar()
            ..showSnackBar(SnackBar(content: Text(next.message)));
        });
      });
    }
    final auth = configurationError == null
        ? ref.watch(authControllerProvider)
        : null;
    Widget guardedRoot() => configurationError != null
        ? ConfigurationRequiredPage(error: configurationError!)
        : auth!.when(
            data: (user) =>
                user == null ? const LoginPage() : const ConnectionGatePage(),
            loading: () => const Scaffold(
              body: CheeseLoadingView(message: '로그인 상태를 확인하는 중입니다'),
            ),
            error: (error, _) =>
                Scaffold(body: Center(child: Text('인증 상태 오류: $error'))),
          );
    return MaterialApp(
      title: 'MOUSEKEEPER',
      scaffoldMessengerKey: mousekeeperScaffoldMessengerKey,
      debugShowCheckedModeBanner: false,
      theme: PixelTheme.light,
      home: guardedRoot(),
      // Every externally supplied named route crosses the same authoritative
      // device gate before a nested main navigator can be created.
      onGenerateRoute: (settings) => settings.name == Navigator.defaultRouteName
          ? null
          : MaterialPageRoute<void>(
              settings: settings,
              builder: (_) => guardedRoot(),
            ),
    );
  }
}

class ConfigurationRequiredPage extends StatelessWidget {
  const ConfigurationRequiredPage({super.key, required this.error});
  final String error;
  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: AppBar(
      title: const FittedBox(
        fit: BoxFit.scaleDown,
        child: Text('MOUSEKEEPER', maxLines: 1, softWrap: false),
      ),
    ),
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
