import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../core/network/api_client.dart';

class RulesPage extends ConsumerStatefulWidget {
  const RulesPage({super.key, required this.roomId});
  final String roomId;

  @override
  ConsumerState<RulesPage> createState() => _RulesPageState();
}

class _RulesPageState extends ConsumerState<RulesPage> {
  late Future<List<Map<String, dynamic>>> _rules;

  @override
  void initState() {
    super.initState();
    _reload();
  }

  void _reload() {
    _rules = ref
        .read(apiClientProvider)
        .getList('/v1/rooms/${widget.roomId}/rules');
  }

  Future<void> _setEnabled(Map<String, dynamic> rule, bool enabled) async {
    try {
      await ref.read(apiClientProvider).patch('/v1/rules/${rule['id']}', {
        'version': rule['version'],
        'enabled': enabled,
      });
    } catch (error) {
      if (mounted) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text('규칙 변경 실패: $error')));
      }
    } finally {
      if (mounted) setState(_reload);
    }
  }

  Future<void> _showRuleDialog([Map<String, dynamic>? existing]) async {
    final definition = existing?['definition'] is Map
        ? Map<String, dynamic>.from(existing!['definition'] as Map)
        : <String, dynamic>{};
    final conditions = definition['conditions'] as List<dynamic>? ?? const [];
    final firstCondition = conditions.isEmpty
        ? <String, dynamic>{}
        : Map<String, dynamic>.from(conditions.first as Map);
    final action = definition['action'] is Map
        ? Map<String, dynamic>.from(definition['action'] as Map)
        : <String, dynamic>{};
    final initialCondition = firstCondition['field'] == 'ageDays'
        ? 'ageDays'
        : 'extension';
    final initialValue = firstCondition['value'];
    final name = TextEditingController(
      text: existing?['name'] as String? ?? '',
    );
    final conditionValue = TextEditingController(
      text: initialValue is List
          ? initialValue.join(', ')
          : initialValue?.toString() ?? '',
    );
    final destination = TextEditingController(
      text: action['destinationTemplate'] as String? ?? 'Archive',
    );
    var condition = initialCondition;
    String? formError;
    final saved = await showDialog<bool>(
      context: context,
      builder: (context) => StatefulBuilder(
        builder: (context, setDialogState) => AlertDialog(
          title: Text(existing == null ? '정리 규칙 만들기' : '정리 규칙 수정'),
          content: SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                TextField(
                  controller: name,
                  decoration: const InputDecoration(labelText: '규칙 이름'),
                ),
                if (formError != null) ...[
                  const SizedBox(height: 12),
                  Text(
                    formError!,
                    style: TextStyle(
                      color: Theme.of(context).colorScheme.error,
                    ),
                  ),
                ],
                const SizedBox(height: 12),
                DropdownButtonFormField<String>(
                  initialValue: condition,
                  decoration: const InputDecoration(labelText: '조건'),
                  items: const [
                    DropdownMenuItem(value: 'extension', child: Text('확장자')),
                    DropdownMenuItem(value: 'ageDays', child: Text('지난 일수')),
                  ],
                  onChanged: (value) {
                    if (value != null) setDialogState(() => condition = value);
                  },
                ),
                const SizedBox(height: 12),
                TextField(
                  controller: conditionValue,
                  keyboardType: condition == 'ageDays'
                      ? TextInputType.number
                      : TextInputType.text,
                  decoration: InputDecoration(
                    labelText: condition == 'ageDays'
                        ? '며칠 이상 지난 파일'
                        : '확장자 (.pdf, .jpg)',
                  ),
                ),
                const SizedBox(height: 12),
                TextField(
                  controller: destination,
                  decoration: const InputDecoration(labelText: '이동할 상대 폴더'),
                ),
              ],
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(context, false),
              child: const Text('취소'),
            ),
            FilledButton(
              onPressed: () async {
                final rawName = name.text.trim();
                final rawDestination = destination.text.trim();
                final rawValue = conditionValue.text.trim();
                if (rawName.isEmpty ||
                    rawDestination.isEmpty ||
                    rawValue.isEmpty) {
                  return;
                }
                final conditionBody = condition == 'ageDays'
                    ? <String, dynamic>{
                        'field': 'ageDays',
                        'operator': 'GTE',
                        'value': int.tryParse(rawValue),
                      }
                    : <String, dynamic>{
                        'field': 'extension',
                        'operator': 'IN',
                        'value': rawValue
                            .split(',')
                            .map((value) => value.trim().toLowerCase())
                            .where((value) => value.isNotEmpty)
                            .toList(),
                      };
                if (conditionBody['value'] == null) return;
                final body = <String, dynamic>{
                  'name': rawName,
                  'definition': {
                    'match': 'ALL',
                    'conditions': [conditionBody],
                    'action': {
                      'type': 'MOVE',
                      'destinationTemplate': rawDestination,
                    },
                  },
                  'priority': existing?['priority'] as int? ?? 100,
                  'enabled': existing?['enabled'] as bool? ?? true,
                };
                try {
                  if (existing == null) {
                    await ref
                        .read(apiClientProvider)
                        .post('/v1/rooms/${widget.roomId}/rules', body);
                  } else {
                    await ref.read(apiClientProvider).patch(
                      '/v1/rules/${existing['id']}',
                      {'version': existing['version'], ...body},
                    );
                  }
                  if (context.mounted) Navigator.pop(context, true);
                } catch (error) {
                  setDialogState(() {
                    formError = error.toString().contains('VERSION_CONFLICT')
                        ? '다른 기기에서 규칙이 변경됐습니다. 목록을 새로고침한 뒤 다시 시도하세요.'
                        : '규칙을 저장하지 못했습니다.';
                  });
                }
              },
              child: const Text('저장'),
            ),
          ],
        ),
      ),
    );
    name.dispose();
    conditionValue.dispose();
    destination.dispose();
    if (saved == true && mounted) setState(_reload);
  }

  String _summary(Map<String, dynamic> rule) {
    final definition = rule['definition'];
    if (definition is! Map<String, dynamic>) return '구조화된 규칙';
    final conditions = definition['conditions'];
    if (conditions is! List || conditions.isEmpty || conditions.first is! Map) {
      return '구조화된 규칙';
    }
    final condition = Map<String, dynamic>.from(conditions.first as Map);
    if (condition['field'] == 'extension') return '확장자 ${condition['value']}';
    if (condition['field'] == 'ageDays') {
      return '${condition['value']}일 이상 지난 파일';
    }
    return '파일 이름 조건';
  }

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: AppBar(title: const Text('정리 규칙')),
    floatingActionButton: FloatingActionButton.extended(
      onPressed: _showRuleDialog,
      icon: const Icon(Icons.add),
      label: const Text('규칙 추가'),
    ),
    body: FutureBuilder<List<Map<String, dynamic>>>(
      future: _rules,
      builder: (context, snapshot) {
        if (snapshot.connectionState != ConnectionState.done) {
          return const Center(child: CircularProgressIndicator());
        }
        if (snapshot.hasError) {
          return Center(child: Text('규칙을 불러오지 못했습니다.\n${snapshot.error}'));
        }
        final rules = snapshot.data ?? const [];
        if (rules.isEmpty) {
          return const Center(child: Text('등록된 규칙이 없습니다.'));
        }
        return RefreshIndicator(
          onRefresh: () async {
            setState(_reload);
            await _rules;
          },
          child: ListView.builder(
            padding: const EdgeInsets.fromLTRB(16, 16, 16, 96),
            itemCount: rules.length,
            itemBuilder: (context, index) {
              final rule = rules[index];
              return Card(
                child: SwitchListTile(
                  secondary: IconButton(
                    tooltip: '규칙 수정',
                    icon: const Icon(Icons.edit_outlined),
                    onPressed: () => _showRuleDialog(rule),
                  ),
                  value: rule['enabled'] == true,
                  onChanged: (value) => _setEnabled(rule, value),
                  title: Text(rule['name'] as String? ?? '규칙'),
                  subtitle: Text('${_summary(rule)} · 버전 ${rule['version']}'),
                ),
              );
            },
          ),
        );
      },
    ),
  );
}
