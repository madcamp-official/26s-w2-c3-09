import 'package:flutter/material.dart';

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  runApp(const HousemouseApp());
}

class HousemouseApp extends StatelessWidget {
  const HousemouseApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'HOUSEMOUSE',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(
          seedColor: const Color(0xFF76543A),
          brightness: Brightness.light,
        ),
        useMaterial3: true,
      ),
      home: const ConfigurationRequiredPage(),
    );
  }
}

class ConfigurationRequiredPage extends StatelessWidget {
  const ConfigurationRequiredPage({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('HOUSEMOUSE')),
      body: const SafeArea(
        child: Center(
          child: Padding(
            padding: EdgeInsets.all(24),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(Icons.settings_outlined, size: 56),
                SizedBox(height: 20),
                Text('Google 로그인이 아직 설정되지 않았습니다.'),
                SizedBox(height: 8),
                Text(
                  'Firebase 설정 파일을 연결하면 로그인 화면을 사용할 수 있습니다.\n오류 코드: UNCONFIGURED',
                  textAlign: TextAlign.center,
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
