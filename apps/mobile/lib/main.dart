import 'package:firebase_core/firebase_core.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'app.dart';
import 'core/config/app_config.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  String? error = AppConfig.validate();
  if (error == null) {
    try {
      await Firebase.initializeApp();
    } catch (_) {
      error = 'Firebase native configuration';
    }
  }
  runApp(ProviderScope(child: HousemouseApp(configurationError: error)));
}
