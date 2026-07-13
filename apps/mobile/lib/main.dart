import 'package:firebase_core/firebase_core.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:sentry_flutter/sentry_flutter.dart';
import 'app.dart';
import 'core/config/app_config.dart';
import 'core/observability/sentry_privacy.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final error = AppConfig.validate();
  if (error != null || AppConfig.sentryDsn.isEmpty) {
    await _initializeAndRun(error: error, sentryEnabled: false);
    return;
  }
  await SentryFlutter.init((options) {
    options.dsn = AppConfig.sentryDsn;
    options.sendDefaultPii = false;
    options.tracesSampleRate = 0;
    options.attachScreenshot = false;
    options.enableUserInteractionTracing = false;
    options.beforeBreadcrumb = (_, _) => null;
    options.beforeSend = scrubMobileSentryEvent;
  }, appRunner: () => _initializeAndRun(error: null, sentryEnabled: true));
}

Future<void> _initializeAndRun({
  required String? error,
  required bool sentryEnabled,
}) async {
  var configurationError = error;
  if (configurationError == null) {
    try {
      await Firebase.initializeApp();
    } catch (_) {
      configurationError = 'Firebase native configuration';
    }
  }
  final app = ProviderScope(
    child: MouseKeeperApp(configurationError: configurationError),
  );
  runApp(sentryEnabled ? SentryWidget(child: app) : app);
}
