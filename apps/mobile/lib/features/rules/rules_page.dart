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

class RuleConditionOption {
  const RuleConditionOption({
    required this.id,
    required this.label,
    required this.field,
    required this.operator,
    required this.valueLabel,
    this.numeric = false,
    this.fileKind = false,
  });

  final String id;
  final String label;
  final String field;
  final String operator;
  final String valueLabel;
  final bool numeric;
  final bool fileKind;
}

const ruleConditionOptions = [
  RuleConditionOption(
    id: 'extension',
    label: '확장자',
    field: 'extension',
    operator: 'IN',
    valueLabel: '확장자 (.pdf, .jpg)',
  ),
  RuleConditionOption(
    id: 'ageDays',
    label: '지난 일수(기존)',
    field: 'ageDays',
    operator: 'GTE',
    valueLabel: '며칠 이상 지난 파일',
    numeric: true,
  ),
  RuleConditionOption(
    id: 'modifiedAgeDaysGte',
    label: '수정일 기준 이상',
    field: 'modifiedAgeDays',
    operator: 'GTE',
    valueLabel: '수정 후 며칠 이상',
    numeric: true,
  ),
  RuleConditionOption(
    id: 'modifiedAgeDaysGt',
    label: '수정일 기준 초과',
    field: 'modifiedAgeDays',
    operator: 'GT',
    valueLabel: '수정 후 며칠 초과',
    numeric: true,
  ),
  RuleConditionOption(
    id: 'createdAgeDaysGte',
    label: '생성일 기준 이상',
    field: 'createdAgeDays',
    operator: 'GTE',
    valueLabel: '생성 후 며칠 이상',
    numeric: true,
  ),
  RuleConditionOption(
    id: 'createdAgeDaysGt',
    label: '생성일 기준 초과',
    field: 'createdAgeDays',
    operator: 'GT',
    valueLabel: '생성 후 며칠 초과',
    numeric: true,
  ),
  RuleConditionOption(
    id: 'sizeBytesGte',
    label: '파일 크기 이상',
    field: 'sizeBytes',
    operator: 'GTE',
    valueLabel: '크기 byte 이상',
    numeric: true,
  ),
  RuleConditionOption(
    id: 'sizeBytesLte',
    label: '파일 크기 이하',
    field: 'sizeBytes',
    operator: 'LTE',
    valueLabel: '크기 byte 이하',
    numeric: true,
  ),
  RuleConditionOption(
    id: 'relativePathStartsWith',
    label: '상대 경로 시작',
    field: 'relativePath',
    operator: 'STARTS_WITH',
    valueLabel: '상대 경로 prefix',
  ),
  RuleConditionOption(
    id: 'fileKind',
    label: '파일 종류',
    field: 'fileKind',
    operator: 'EQ',
    valueLabel: '파일 또는 폴더',
    fileKind: true,
  ),
  RuleConditionOption(
    id: 'nameContains',
    label: '이름 포함',
    field: 'name',
    operator: 'CONTAINS',
    valueLabel: '포함할 이름',
  ),
  RuleConditionOption(
    id: 'nameStartsWith',
    label: '이름 시작',
    field: 'name',
    operator: 'STARTS_WITH',
    valueLabel: '시작 문자열',
  ),
  RuleConditionOption(
    id: 'nameEndsWith',
    label: '이름 끝',
    field: 'name',
    operator: 'ENDS_WITH',
    valueLabel: '끝 문자열',
  ),
];

const ruleActionLabels = {
  'MOVE': '이동',
  'TRASH': '휴지통',
  'CREATE_DIR': '폴더 만들기',
  'QUARANTINE': '격리(기존)',
};

RuleConditionOption ruleConditionOption(String id) =>
    ruleConditionOptions.firstWhere(
      (option) => option.id == id,
      orElse: () => ruleConditionOptions.first,
    );

String ruleConditionIdFrom(Map<String, dynamic> condition) {
  final field = condition['field'];
  final operator = condition['operator'];
  return ruleConditionOptions
      .firstWhere(
        (option) => option.field == field && option.operator == operator,
        orElse: () => ruleConditionOptions.first,
      )
      .id;
}

Map<String, dynamic>? ruleConditionBody({
  required String conditionId,
  required String rawValue,
  required String fileKindValue,
}) {
  final option = ruleConditionOption(conditionId);
  if (option.fileKind) {
    return {
      'field': option.field,
      'operator': option.operator,
      'value': fileKindValue,
    };
  }
  final trimmed = rawValue.trim();
  if (trimmed.isEmpty) return null;
  if (option.id == 'extension') {
    final values = trimmed
        .split(',')
        .map((value) => value.trim().toLowerCase())
        .where((value) => value.isNotEmpty)
        .toList();
    if (values.isEmpty) return null;
    return {
      'field': option.field,
      'operator': option.operator,
      'value': values,
    };
  }
  if (option.numeric) {
    final value = int.tryParse(trimmed);
    if (value == null) return null;
    return {'field': option.field, 'operator': option.operator, 'value': value};
  }
  return {'field': option.field, 'operator': option.operator, 'value': trimmed};
}

Map<String, dynamic>? ruleActionBody({
  required String actionType,
  required String rawPath,
}) {
  final trimmed = rawPath.trim();
  return switch (actionType) {
    'MOVE' when trimmed.isNotEmpty => {
      'type': 'MOVE',
      'destinationTemplate': trimmed,
    },
    'CREATE_DIR' when trimmed.isNotEmpty => {
      'type': 'CREATE_DIR',
      'relativePath': trimmed,
    },
    'TRASH' => {'type': 'TRASH'},
    'QUARANTINE' => {'type': 'QUARANTINE'},
    _ => null,
  };
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
    final initialCondition = ruleConditionIdFrom(firstCondition);
    final initialValue = firstCondition['value'];
    final initialActionType = ruleActionLabels.containsKey(action['type'])
        ? action['type'] as String
        : 'MOVE';
    final name = TextEditingController(
      text: existing?['name'] as String? ?? '',
    );
    final conditionValue = TextEditingController(
      text: initialValue is List
          ? initialValue.join(', ')
          : firstCondition['field'] == 'fileKind'
          ? ''
          : initialValue?.toString() ?? '',
    );
    final destination = TextEditingController(
      text:
          action['destinationTemplate'] as String? ??
          action['relativePath'] as String? ??
          'Archive',
    );
    var condition = initialCondition;
    var fileKindValue = firstCondition['value'] == 'DIRECTORY'
        ? 'DIRECTORY'
        : 'FILE';
    var actionType = initialActionType;
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
                  key: const ValueKey('rule-condition-field'),
                  initialValue: condition,
                  decoration: const InputDecoration(labelText: '조건'),
                  items: [
                    for (final option in ruleConditionOptions)
                      DropdownMenuItem(
                        value: option.id,
                        child: Text(option.label),
                      ),
                  ],
                  onChanged: (value) {
                    if (value != null) setDialogState(() => condition = value);
                  },
                ),
                const SizedBox(height: 12),
                if (ruleConditionOption(condition).fileKind)
                  DropdownButtonFormField<String>(
                    key: const ValueKey('rule-file-kind-field'),
                    initialValue: fileKindValue,
                    decoration: const InputDecoration(labelText: '파일 종류'),
                    items: const [
                      DropdownMenuItem(value: 'FILE', child: Text('파일')),
                      DropdownMenuItem(value: 'DIRECTORY', child: Text('폴더')),
                    ],
                    onChanged: (value) {
                      if (value != null) {
                        setDialogState(() => fileKindValue = value);
                      }
                    },
                  )
                else
                  TextField(
                    key: const ValueKey('rule-condition-value-field'),
                    controller: conditionValue,
                    keyboardType: ruleConditionOption(condition).numeric
                        ? TextInputType.number
                        : TextInputType.text,
                    decoration: InputDecoration(
                      labelText: ruleConditionOption(condition).valueLabel,
                    ),
                  ),
                const SizedBox(height: 12),
                DropdownButtonFormField<String>(
                  key: const ValueKey('rule-action-field'),
                  initialValue: actionType,
                  decoration: const InputDecoration(labelText: '동작'),
                  items: [
                    for (final entry in ruleActionLabels.entries)
                      DropdownMenuItem(
                        value: entry.key,
                        child: Text(entry.value),
                      ),
                  ],
                  onChanged: (value) {
                    if (value != null) {
                      setDialogState(() => actionType = value);
                    }
                  },
                ),
                if (actionType == 'MOVE' || actionType == 'CREATE_DIR') ...[
                  const SizedBox(height: 12),
                  TextField(
                    key: const ValueKey('rule-destination-field'),
                    controller: destination,
                    decoration: InputDecoration(
                      labelText: actionType == 'CREATE_DIR'
                          ? '만들 상대 폴더'
                          : '이동할 상대 폴더',
                    ),
                  ),
                ],
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
                if (rawName.isEmpty) {
                  setDialogState(() {
                    formError = '규칙 이름을 입력해 주세요.';
                  });
                  return;
                }
                final conditionBody = ruleConditionBody(
                  conditionId: condition,
                  rawValue: rawValue,
                  fileKindValue: fileKindValue,
                );
                final actionBody = ruleActionBody(
                  actionType: actionType,
                  rawPath: rawDestination,
                );
                if (conditionBody == null) {
                  setDialogState(() => formError = '조건 값을 확인해 주세요.');
                  return;
                }
                if (actionBody == null) {
                  setDialogState(() => formError = '동작과 상대 경로를 확인해 주세요.');
                  return;
                }
                final body = <String, dynamic>{
                  'name': rawName,
                  'definition': {
                    'match': 'ALL',
                    'conditions': [conditionBody],
                    'action': actionBody,
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
    if (condition['field'] == 'modifiedAgeDays') {
      return '수정 후 ${condition['value']}일 ${condition['operator']}';
    }
    if (condition['field'] == 'createdAgeDays') {
      return '생성 후 ${condition['value']}일 ${condition['operator']}';
    }
    if (condition['field'] == 'sizeBytes') {
      return '크기 ${condition['operator']} ${condition['value']} bytes';
    }
    if (condition['field'] == 'relativePath') {
      return '경로가 ${condition['value']}로 시작';
    }
    if (condition['field'] == 'fileKind') return '종류 ${condition['value']}';
    if (condition['field'] == 'name') {
      return '이름 ${condition['operator']} ${condition['value']}';
    }
    return '구조화된 규칙';
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
