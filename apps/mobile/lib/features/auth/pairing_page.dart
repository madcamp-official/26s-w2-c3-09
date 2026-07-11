import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../core/network/api_client.dart';

class PairingPage extends ConsumerStatefulWidget {
  const PairingPage({super.key});
  @override
  ConsumerState<PairingPage> createState() => _PairingPageState();
}

class _PairingPageState extends ConsumerState<PairingPage> {
  final controller = TextEditingController();
  bool submitting = false;
  @override
  void dispose() {
    controller.dispose();
    super.dispose();
  }

  Future<void> claim() async {
    if (controller.text.length != 6) return;
    setState(() => submitting = true);
    try {
      await ref.read(apiClientProvider).post('/v1/pairing-sessions/claim', {
        'code': controller.text,
      });
      if (mounted) Navigator.of(context).pop(true);
    } catch (error) {
      if (mounted) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text('페어링 실패: $error')));
      }
    } finally {
      if (mounted) setState(() => submitting = false);
    }
  }

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: AppBar(title: const Text('PC 연결')),
    body: Padding(
      padding: const EdgeInsets.all(24),
      child: Column(
        children: [
          const Text('데스크톱 앱에 표시된 6자리 코드를 입력하세요.'),
          const SizedBox(height: 20),
          TextField(
            controller: controller,
            keyboardType: TextInputType.number,
            maxLength: 6,
            inputFormatters: [FilteringTextInputFormatter.digitsOnly],
            decoration: const InputDecoration(
              labelText: '페어링 코드',
              border: OutlineInputBorder(),
            ),
          ),
          const SizedBox(height: 12),
          SizedBox(
            width: double.infinity,
            child: FilledButton(
              onPressed: submitting ? null : claim,
              child: const Text('연결'),
            ),
          ),
        ],
      ),
    ),
  );
}
