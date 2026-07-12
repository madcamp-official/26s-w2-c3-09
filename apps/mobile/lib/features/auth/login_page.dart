import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:housemouse_character_assets/character_assets.dart';
import 'auth_controller.dart';

class LoginPage extends ConsumerWidget {
  const LoginPage({super.key});
  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final auth = ref.watch(authControllerProvider);
    return Scaffold(
      body: SafeArea(
        child: Center(
          child: Padding(
            padding: const EdgeInsets.all(24),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                SvgPicture.asset(
                  housemouseMascotAsset,
                  package: housemouseMascotPackage,
                  width: 180,
                  height: 180,
                  semanticsLabel: 'HouseMouse 마스코트',
                ),
                const SizedBox(height: 16),
                Text(
                  'HOUSEMOUSE',
                  style: Theme.of(context).textTheme.headlineMedium,
                ),
                const SizedBox(height: 8),
                const Text('PC의 폴더를 안전하게 돌보는 집쥐인'),
                const SizedBox(height: 28),
                FilledButton.icon(
                  onPressed: auth.isLoading
                      ? null
                      : () => ref
                            .read(authControllerProvider.notifier)
                            .signInWithGoogle(),
                  icon: const Icon(Icons.login),
                  label: const Text('Google로 로그인'),
                ),
                if (auth.hasError)
                  Padding(
                    padding: const EdgeInsets.only(top: 16),
                    child: Text(
                      '로그인 오류: ${auth.error}',
                      style: TextStyle(
                        color: Theme.of(context).colorScheme.error,
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
