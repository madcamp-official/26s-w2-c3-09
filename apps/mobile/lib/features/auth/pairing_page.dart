import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/network/api_client.dart';
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
    if (controller.text.length != 6) return;
    setState(() {
      submitting = true;
      failure = null;
    });
    try {
      final claim = widget.onClaim;
      if (claim == null) {
        await ref.read(apiClientProvider).post('/v1/pairing-sessions/claim', {
          'code': controller.text,
        });
      } else {
        await claim(controller.text);
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
      child: Center(
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              const Icon(Icons.phonelink_outlined, size: 72),
              const SizedBox(height: 18),
              Text(
                'PC와 연결하기',
                textAlign: TextAlign.center,
                style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                  fontWeight: FontWeight.w800,
                ),
              ),
              const SizedBox(height: 8),
              const Text(
                '데스크탑 앱에 표시된 6자리 코드를 입력해 주세요.\n'
                '페어링 전에는 다른 메뉴가 열리지 않습니다.',
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 24),
              TextField(
                controller: controller,
                keyboardType: TextInputType.number,
                maxLength: 6,
                textAlign: TextAlign.center,
                style: Theme.of(context).textTheme.headlineMedium?.copyWith(
                  letterSpacing: 8,
                  fontWeight: FontWeight.w800,
                ),
                inputFormatters: [FilteringTextInputFormatter.digitsOnly],
                onChanged: (_) => setState(() => failure = null),
                decoration: const InputDecoration(
                  labelText: '페어링 코드',
                  border: OutlineInputBorder(),
                  counterText: '',
                ),
              ),
              if (failure != null) ...[
                const SizedBox(height: 12),
                Card(
                  color: Theme.of(context).colorScheme.errorContainer,
                  child: ListTile(
                    leading: const Icon(Icons.error_outline),
                    title: const Text('PC 연결을 확인하지 못했습니다'),
                    subtitle: Text(failure!),
                  ),
                ),
              ],
              const SizedBox(height: 16),
              FilledButton(
                onPressed: submitting || controller.text.length != 6
                    ? null
                    : claim,
                child: submitting
                    ? const Row(
                        mainAxisAlignment: MainAxisAlignment.center,
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          SizedBox.square(
                            dimension: 18,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          ),
                          SizedBox(width: 10),
                          Text('연결 확인 중'),
                        ],
                      )
                    : const Text('연결'),
              ),
              if (widget.gateMode) ...[
                const SizedBox(height: 12),
                TextButton(
                  onPressed: submitting
                      ? null
                      : () =>
                            ref.read(authControllerProvider.notifier).signOut(),
                  child: const Text('다른 계정으로 로그인'),
                ),
              ],
            ],
          ),
        ),
      ),
    ),
  );
}
