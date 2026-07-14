import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'features/auth/auth_controller.dart';
import 'features/auth/connection_gate_page.dart';
import 'features/auth/login_page.dart';
import 'core/sync/realtime_controller.dart';

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
              body: Center(child: CircularProgressIndicator()),
            ),
            error: (error, _) =>
                Scaffold(body: Center(child: Text('인증 상태 오류: $error'))),
          );
    return MaterialApp(
      title: 'MOUSEKEEPER',
      scaffoldMessengerKey: mousekeeperScaffoldMessengerKey,
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(
          seedColor: const Color(0xFF76543A),
          brightness: Brightness.light,
          surface: const Color(0xFFFFFAF4),
        ),
        scaffoldBackgroundColor: const Color(0xFFF4E9DC),
        appBarTheme: const AppBarTheme(
          backgroundColor: Color(0xFFF4E9DC),
          foregroundColor: Color(0xFF3B2A24),
          surfaceTintColor: Colors.transparent,
          elevation: 0,
        ),
        cardTheme: CardThemeData(
          color: const Color(0xFFFFFAF4),
          surfaceTintColor: Colors.transparent,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(18),
            side: const BorderSide(color: Color(0xFFB9A696)),
          ),
        ),
        dialogTheme: DialogThemeData(
          backgroundColor: const Color(0xFFFFFAF4),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(18),
            side: const BorderSide(color: Color(0xFFB9A696)),
          ),
        ),
        filledButtonTheme: FilledButtonThemeData(
          style: FilledButton.styleFrom(
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(16),
            ),
            textStyle: const TextStyle(fontWeight: FontWeight.w800),
          ),
        ),
        outlinedButtonTheme: OutlinedButtonThemeData(
          style: OutlinedButton.styleFrom(
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(16),
            ),
            textStyle: const TextStyle(fontWeight: FontWeight.w800),
          ),
        ),
        useMaterial3: true,
      ),
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
