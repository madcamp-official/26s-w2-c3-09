import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:mousekeeper_character_assets/character_assets.dart';

import '../../core/network/api_client.dart';
import '../../core/theme/pixel_theme.dart';
import 'auth_controller.dart';

typedef PairingClaim = Future<void> Function(String code);

class PairingPage extends ConsumerStatefulWidget {
  const PairingPage({super.key, this.onClaim, this.gateMode = false});

  final PairingClaim? onClaim;
  final bool gateMode;

  @override
  ConsumerState<PairingPage> createState() => _PairingPageState();
}

class _PairingPageState extends ConsumerState<PairingPage> {
  final controller = TextEditingController();
  bool submitting = false;
  String? failure;

  @override
  void dispose() {
    controller.dispose();
    super.dispose();
  }

  Future<void> claim() async {
    final code = controller.text.trim();
    if (code.length != 6 || submitting) return;
    setState(() {
      submitting = true;
      failure = null;
    });
    try {
      final claim = widget.onClaim;
      if (claim == null) {
        await ref.read(apiClientProvider).post('/v1/pairing-sessions/claim', {
          'code': code,
        });
      } else {
        await claim(code);
      }
      if (mounted && !widget.gateMode) Navigator.of(context).pop(true);
    } catch (error) {
      if (mounted) {
        setState(() => failure = '연결 확인 실패: $error');
      }
    } finally {
      if (mounted) setState(() => submitting = false);
    }
  }

  @override
  Widget build(BuildContext context) => Scaffold(
    body: SafeArea(
      child: LayoutBuilder(
        builder: (context, constraints) => SingleChildScrollView(
          child: ConstrainedBox(
            constraints: BoxConstraints(minHeight: constraints.maxHeight),
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 34, vertical: 28),
              child: PixelPanel(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    const Center(child: PixelLabel('PAIR MODE')),
                    const SizedBox(height: 18),
                    Text(
                      'MOUSEKEEPER',
                      textAlign: TextAlign.center,
                      style: Theme.of(context).textTheme.displaySmall?.copyWith(
                        color: PixelColors.ink,
                        fontWeight: FontWeight.w900,
                        letterSpacing: 1.2,
                      ),
                    ),
                    const SizedBox(height: 34),
                    Image.asset(
                      mousekeeperPairingIconAsset,
                      package: mousekeeperMascotPackage,
                      width: 86,
                      height: 86,
                      fit: BoxFit.contain,
                      filterQuality: FilterQuality.none,
                    ),
                    const SizedBox(height: 12),
                    Text(
                      '기기를 연결해주세요',
                      textAlign: TextAlign.center,
                      style: Theme.of(context).textTheme.headlineMedium
                          ?.copyWith(
                            color: const Color(0xFF1E1717),
                            fontWeight: FontWeight.w500,
                            letterSpacing: -1.2,
                          ),
                    ),
                    const SizedBox(height: 4),
                    Text(
                      'Desktop의 코드를 입력해주세요',
                      textAlign: TextAlign.center,
                      style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                        color: const Color(0xFF2D1F1F),
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                    const SizedBox(height: 16),
                    Center(
                      child: ConstrainedBox(
                        constraints: const BoxConstraints(maxWidth: 332),
                        child: TextField(
                          controller: controller,
                          enabled: !submitting,
                          autofocus: true,
                          keyboardType: TextInputType.number,
                          maxLength: 6,
                          textAlign: TextAlign.left,
                          style: Theme.of(context).textTheme.headlineMedium
                              ?.copyWith(
                                color: PixelColors.ink,
                                fontWeight: FontWeight.w700,
                                letterSpacing: 3,
                              ),
                          inputFormatters: [
                            FilteringTextInputFormatter.digitsOnly,
                            LengthLimitingTextInputFormatter(6),
                          ],
                          onChanged: (value) {
                            if (failure != null) setState(() => failure = null);
                            if (value.length == 6) unawaited(claim());
                          },
                          decoration: const InputDecoration(
                            hintText: '6자리 코드',
                            counterText: '',
                            filled: true,
                            fillColor: PixelColors.paper,
                            contentPadding: EdgeInsets.symmetric(
                              horizontal: 22,
                              vertical: 18,
                            ),
                            enabledBorder: OutlineInputBorder(
                              borderRadius: BorderRadius.zero,
                              borderSide: BorderSide(
                                color: PixelColors.ink,
                                width: 2,
                              ),
                            ),
                            focusedBorder: OutlineInputBorder(
                              borderRadius: BorderRadius.zero,
                              borderSide: BorderSide(
                                color: PixelColors.brown,
                                width: 3,
                              ),
                            ),
                            disabledBorder: OutlineInputBorder(
                              borderRadius: BorderRadius.zero,
                              borderSide: BorderSide(
                                color: PixelColors.muted,
                                width: 2,
                              ),
                            ),
                          ),
                        ),
                      ),
                    ),
                    const SizedBox(height: 16),
                    AnimatedSwitcher(
                      duration: const Duration(milliseconds: 160),
                      child: submitting
                          ? Row(
                              key: const ValueKey('pairing-submitting'),
                              mainAxisAlignment: MainAxisAlignment.center,
                              children: [
                                const SizedBox.square(
                                  dimension: 16,
                                  child: CircularProgressIndicator(
                                    strokeWidth: 2,
                                  ),
                                ),
                                const SizedBox(width: 8),
                                Text(
                                  '연결 확인 중입니다',
                                  style: Theme.of(context).textTheme.bodyMedium
                                      ?.copyWith(
                                        color: const Color(0xFF7A685C),
                                        fontWeight: FontWeight.w700,
                                      ),
                                ),
                              ],
                            )
                          : Text(
                              key: const ValueKey('pairing-expiry-notice'),
                              '코드는 데스크톱 화면에 표시된 시간 후 만료됩니다',
                              textAlign: TextAlign.center,
                              style: Theme.of(context).textTheme.bodyMedium
                                  ?.copyWith(
                                    color: const Color(0xFF7A685C),
                                    fontWeight: FontWeight.w700,
                                  ),
                            ),
                    ),
                    if (failure != null) ...[
                      const SizedBox(height: 14),
                      _PairingFailureNotice(message: failure!),
                    ],
                    if (widget.gateMode) ...[
                      const SizedBox(height: 22),
                      TextButton(
                        onPressed: submitting
                            ? null
                            : () => ref
                                  .read(authControllerProvider.notifier)
                                  .signOut(),
                        child: const Text('다른 계정으로 로그인'),
                      ),
                    ],
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    ),
  );
}

class _PairingFailureNotice extends StatelessWidget {
  const _PairingFailureNotice({required this.message});

  final String message;

  @override
  Widget build(BuildContext context) => DecoratedBox(
    decoration: BoxDecoration(
      color: Theme.of(context).colorScheme.errorContainer,
      border: Border.all(color: PixelColors.ink, width: 2),
    ),
    child: Padding(
      padding: const EdgeInsets.all(14),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Icon(Icons.error_outline, size: 20),
          const SizedBox(width: 10),
          Expanded(
            child: Text(
              'PC 연결을 확인하지 못했습니다\n$message',
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ),
        ],
      ),
    ),
  );
}
