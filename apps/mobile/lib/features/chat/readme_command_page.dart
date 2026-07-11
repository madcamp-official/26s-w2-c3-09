import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';
import '../../core/sync/mutation_queue.dart';

typedef ReadmeCommandSubmit =
    Future<bool> Function(Map<String, dynamic> command);

class ReadmeCommandPage extends ConsumerStatefulWidget {
  const ReadmeCommandPage({super.key, required this.roomId, this.submit});

  final String roomId;
  final ReadmeCommandSubmit? submit;

  @override
  ConsumerState<ReadmeCommandPage> createState() => _ReadmeCommandPageState();
}

class _ReadmeCommandPageState extends ConsumerState<ReadmeCommandPage> {
  final _formKey = GlobalKey<FormState>();
  final _purpose = TextEditingController();
  final _audience = TextEditingController();
  final _sections = TextEditingController();
  String _tone = 'concise';
  bool _submitting = false;

  @override
  void dispose() {
    _purpose.dispose();
    _audience.dispose();
    _sections.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: AppBar(title: const Text('README 초안 요청')),
    body: Form(
      key: _formKey,
      child: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          const Card(
            color: Color(0xFFE3F2FD),
            child: ListTile(
              leading: Icon(Icons.verified_user_outlined),
              title: Text('승인 전에는 파일을 변경하지 않습니다'),
              subtitle: Text(
                'PC가 기존 README를 읽고 hash와 실제 diff를 제안하면 모바일에서 검토한 뒤 승인합니다.',
              ),
            ),
          ),
          const SizedBox(height: 16),
          TextFormField(
            controller: _purpose,
            maxLength: 500,
            decoration: const InputDecoration(
              labelText: '이 폴더의 목적',
              hintText: '예: 팀이 공유하는 모바일 앱 프로젝트',
              border: OutlineInputBorder(),
            ),
            validator: _required,
          ),
          const SizedBox(height: 12),
          TextFormField(
            controller: _audience,
            maxLength: 200,
            decoration: const InputDecoration(
              labelText: '주요 독자',
              hintText: '예: 처음 프로젝트를 실행하는 팀원',
              border: OutlineInputBorder(),
            ),
            validator: _required,
          ),
          const SizedBox(height: 12),
          DropdownButtonFormField<String>(
            initialValue: _tone,
            decoration: const InputDecoration(
              labelText: '문체',
              border: OutlineInputBorder(),
            ),
            items: const [
              DropdownMenuItem(value: 'concise', child: Text('간결하게')),
              DropdownMenuItem(value: 'friendly', child: Text('친절하게')),
              DropdownMenuItem(value: 'technical', child: Text('기술적으로')),
            ],
            onChanged: _submitting
                ? null
                : (value) => setState(() => _tone = value!),
          ),
          const SizedBox(height: 12),
          TextFormField(
            controller: _sections,
            minLines: 3,
            maxLines: 8,
            maxLength: 1000,
            decoration: const InputDecoration(
              labelText: '꼭 포함할 항목 (한 줄에 하나)',
              hintText: '설치 방법\n실행 방법\n폴더 구조',
              border: OutlineInputBorder(),
            ),
          ),
          const SizedBox(height: 8),
          FilledButton.icon(
            onPressed: _submitting ? null : _submit,
            icon: _submitting
                ? const SizedBox.square(
                    dimension: 18,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.description_outlined),
            label: const Text('README 제안 요청'),
          ),
        ],
      ),
    ),
  );

  String? _required(String? value) =>
      value == null || value.trim().isEmpty ? '필수 항목입니다' : null;

  Future<void> _submit() async {
    if (!_formKey.currentState!.validate()) return;
    setState(() => _submitting = true);
    final command = <String, dynamic>{
      'intent': 'README',
      'payload': {
        'purpose': _purpose.text.trim(),
        'audience': _audience.text.trim(),
        'tone': _tone,
        'sections': _sections.text
            .split('\n')
            .map((line) => line.trim())
            .where((line) => line.isNotEmpty)
            .toList(),
      },
    };
    try {
      final queued = widget.submit != null
          ? await widget.submit!(command)
          : (await ref
                    .read(mutationQueueProvider)
                    .postOrQueue(
                      mutationType: 'CREATE_COMMAND',
                      path: '/v1/rooms/${widget.roomId}/commands',
                      body: command,
                      idempotencyKey: const Uuid().v4(),
                    ))
                .queued;
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            queued ? '오프라인 요청함에 저장했습니다. 연결되면 전송합니다.' : 'PC에 README 분석을 요청했습니다.',
          ),
        ),
      );
      Navigator.pop(context, true);
    } catch (error) {
      if (mounted) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text('README 요청 실패: $error')));
        setState(() => _submitting = false);
      }
    }
  }
}
