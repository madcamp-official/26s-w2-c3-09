import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/network/api_client.dart';

abstract interface class RuleGateway {
  Future<List<Map<String, dynamic>>> listRules(String roomId);
  Future<Map<String, dynamic>> createRule(
    String roomId,
    Map<String, dynamic> body,
  );
  Future<Map<String, dynamic>> updateRule(
    String ruleId,
    Map<String, dynamic> body,
  );
}

class ApiRuleGateway implements RuleGateway {
  ApiRuleGateway(this._api);

  final ApiClient _api;

  @override
  Future<List<Map<String, dynamic>>> listRules(String roomId) =>
      _api.getList('/v1/rooms/$roomId/rules');

  @override
  Future<Map<String, dynamic>> createRule(
    String roomId,
    Map<String, dynamic> body,
  ) => _api.post('/v1/rooms/$roomId/rules', body);

  @override
  Future<Map<String, dynamic>> updateRule(
    String ruleId,
    Map<String, dynamic> body,
  ) => _api.patch('/v1/rules/$ruleId', body);
}

List<Map<String, dynamic>> upsertRule(
  List<Map<String, dynamic>> rules,
  Map<String, dynamic> updated,
) {
  final updatedId = updated['id'];
  final replaced =
      updatedId != null && rules.any((rule) => rule['id'] == updatedId);
  final next = replaced
      ? [
          for (final rule in rules)
            if (rule['id'] == updatedId) updated else rule,
        ]
      : [updated, ...rules];
  return sortRules(next);
}

List<Map<String, dynamic>> sortRules(List<Map<String, dynamic>> rules) {
  final next = [...rules];
  next.sort((left, right) {
    final priorityCompare = ((left['priority'] as int?) ?? 100).compareTo(
      (right['priority'] as int?) ?? 100,
    );
    if (priorityCompare != 0) return priorityCompare;
    final leftCreated = left['createdAt'] as String? ?? '';
    final rightCreated = right['createdAt'] as String? ?? '';
    return leftCreated.compareTo(rightCreated);
  });
  return next;
}

String ruleMutationErrorMessage(Object error) {
  final raw = error.toString();
  if (raw.contains('VERSION_CONFLICT')) {
    return '다른 기기에서 규칙이 변경됐습니다. 새로고침한 뒤 다시 시도하세요.';
  }
  if (raw.contains('ROOM_REMOVED') || raw.contains('NOT_FOUND')) {
    return '연결 해제되었거나 찾을 수 없는 규칙입니다.';
  }
  return '규칙 작업을 완료하지 못했습니다.';
}

class RulesPage extends ConsumerStatefulWidget {
  const RulesPage({super.key, required this.roomId, this.gateway});

  final String roomId;
  final RuleGateway? gateway;

  @override
  ConsumerState<RulesPage> createState() => _RulesPageState();
}

class _RulesPageState extends ConsumerState<RulesPage> {
  List<Map<String, dynamic>> _rules = const [];
  Object? _error;
  bool _loading = true;
  bool _refreshing = false;
  final Set<String> _updatingRuleIds = {};
  bool _disposed = false;
  int _loadVersion = 0;

  RuleGateway get _gateway =>
      widget.gateway ?? ApiRuleGateway(ref.read(apiClientProvider));

  @override
  void initState() {
    super.initState();
    unawaited(_loadRules());
  }

  @override
  void dispose() {
    _disposed = true;
    _loadVersion++;
    super.dispose();
  }

  Future<void> _loadRules({bool manual = false}) async {
    final version = ++_loadVersion;
    setState(() {
      if (manual) {
        _refreshing = true;
      } else {
        _loading = true;
      }
      _error = null;
    });
    try {
      final rules = await _gateway.listRules(widget.roomId);
      if (_stale(version)) return;
      setState(() {
        _rules = sortRules(rules);
        _loading = false;
        _refreshing = false;
      });
    } catch (error) {
      if (_stale(version)) return;
      setState(() {
        _error = error;
        _loading = false;
        _refreshing = false;
      });
    }
  }

  Future<void> _setEnabled(Map<String, dynamic> rule, bool enabled) async {
    final ruleId = rule['id'] as String?;
    if (ruleId == null || _updatingRuleIds.contains(ruleId)) return;
    setState(() => _updatingRuleIds.add(ruleId));
    try {
      final updated = await _gateway.updateRule(ruleId, {
        'version': rule['version'],
        'enabled': enabled,
      });
      if (_disposed) return;
      setState(() => _rules = upsertRule(_rules, updated));
    } catch (error) {
      _showSnack('규칙 변경 실패: ${ruleMutationErrorMessage(error)}');
    } finally {
      if (!_disposed) {
        setState(() => _updatingRuleIds.remove(ruleId));
      }
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
    final saved = await showDialog<Map<String, dynamic>>(
      context: context,
      builder: (context) => StatefulBuilder(
        builder: (context, setDialogState) => AlertDialog(
          title: Text(existing == null ? '정리 규칙 만들기' : '정리 규칙 수정'),
          content: SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                TextField(
                  key: const ValueKey('rule-name-field'),
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
                  key: const ValueKey('rule-condition-value-field'),
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
                  key: const ValueKey('rule-destination-field'),
                  controller: destination,
                  decoration: const InputDecoration(labelText: '이동할 상대 폴더'),
                ),
              ],
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(context),
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
                  setDialogState(() {
                    formError = '규칙 이름, 조건, 목적지를 모두 입력해 주세요.';
                  });
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
                if (conditionBody['value'] == null) {
                  setDialogState(() => formError = '조건 값을 확인해 주세요.');
                  return;
                }
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
                  final saved = existing == null
                      ? await _gateway.createRule(widget.roomId, body)
                      : await _gateway.updateRule(existing['id'] as String, {
                          'version': existing['version'],
                          ...body,
                        });
                  if (context.mounted) Navigator.pop(context, saved);
                } catch (error) {
                  setDialogState(() {
                    formError = ruleMutationErrorMessage(error);
                  });
                }
              },
              child: const Text('저장'),
            ),
          ],
        ),
      ),
    );
    WidgetsBinding.instance.addPostFrameCallback((_) {
      name.dispose();
      conditionValue.dispose();
      destination.dispose();
    });
    if (saved != null && mounted) {
      setState(() => _rules = upsertRule(_rules, saved));
    }
  }

  bool _stale(int version) => _disposed || version != _loadVersion;

  void _showSnack(String message) {
    if (!_disposed && mounted) {
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(message)));
    }
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
    appBar: AppBar(
      title: const Text('정리 규칙'),
      actions: [
        IconButton(
          tooltip: '새로고침',
          onPressed: _refreshing
              ? null
              : () => unawaited(_loadRules(manual: true)),
          icon: _refreshing
              ? const SizedBox.square(
                  dimension: 20,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Icon(Icons.refresh),
        ),
      ],
    ),
    floatingActionButton: FloatingActionButton.extended(
      onPressed: _showRuleDialog,
      icon: const Icon(Icons.add),
      label: const Text('규칙 추가'),
    ),
    body: _buildBody(),
  );

  Widget _buildBody() {
    if (_loading) return const Center(child: CircularProgressIndicator());
    if (_error != null) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              '규칙을 불러오지 못했습니다.\n${ruleMutationErrorMessage(_error!)}',
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 12),
            FilledButton(
              onPressed: () => unawaited(_loadRules()),
              child: const Text('다시 시도'),
            ),
          ],
        ),
      );
    }
    if (_rules.isEmpty) {
      return RefreshIndicator(
        onRefresh: () => _loadRules(manual: true),
        child: ListView(
          physics: const AlwaysScrollableScrollPhysics(),
          children: const [
            SizedBox(height: 240),
            Center(child: Text('등록된 규칙이 없습니다.')),
          ],
        ),
      );
    }
    return RefreshIndicator(
      onRefresh: () => _loadRules(manual: true),
      child: ListView.builder(
        padding: const EdgeInsets.fromLTRB(16, 16, 16, 96),
        itemCount: _rules.length,
        itemBuilder: (context, index) {
          final rule = _rules[index];
          final ruleId = rule['id'] as String?;
          final updating = ruleId != null && _updatingRuleIds.contains(ruleId);
          return Card(
            child: SwitchListTile(
              key: ValueKey('rule-enabled-$ruleId'),
              secondary: IconButton(
                key: ValueKey('rule-edit-$ruleId'),
                tooltip: '규칙 수정',
                icon: const Icon(Icons.edit_outlined),
                onPressed: updating ? null : () => _showRuleDialog(rule),
              ),
              value: rule['enabled'] == true,
              onChanged: updating ? null : (value) => _setEnabled(rule, value),
              title: Text(rule['name'] as String? ?? '규칙'),
              subtitle: Text('${_summary(rule)} · 버전 ${rule['version']}'),
            ),
          );
        },
      ),
    );
  }
}
