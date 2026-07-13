import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/features/rules/rules_page.dart';

void main() {
  testWidgets('toggle updates only the changed rule without reloading list', (
    tester,
  ) async {
    final gateway = _FakeRuleGateway([
      _rule('r1', 'PDF 정리', enabled: true, version: 1),
      _rule('r2', '이미지 정리', enabled: true, version: 3, extension: '.jpg'),
    ]);

    await _pumpRules(tester, gateway);
    expect(find.text('PDF 정리'), findsOneWidget);
    expect(find.text('이미지 정리'), findsOneWidget);
    expect(gateway.listLoads, 1);

    await tester.tap(find.byKey(const ValueKey('rule-enabled-r1')));
    await tester.pumpAndSettle();

    expect(gateway.listLoads, 1);
    expect(gateway.updateBodies, [
      {'ruleId': 'r1', 'version': 1, 'enabled': false},
    ]);
    expect(find.text('확장자 [.pdf] · 버전 2'), findsOneWidget);
    expect(find.text('확장자 [.jpg] · 버전 3'), findsOneWidget);
  });

  testWidgets('creating a rule upserts server response without full reload', (
    tester,
  ) async {
    final gateway = _FakeRuleGateway([]);

    await _pumpRules(tester, gateway);
    expect(find.text('등록된 규칙이 없습니다.'), findsOneWidget);

    await tester.tap(find.text('규칙 추가'));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const ValueKey('rule-name-field')),
      '문서 정리',
    );
    await tester.enterText(
      find.byKey(const ValueKey('rule-condition-value-field')),
      '.pdf',
    );
    await tester.enterText(
      find.byKey(const ValueKey('rule-destination-field')),
      'Documents',
    );
    await tester.tap(find.text('저장'));
    await tester.pumpAndSettle();

    expect(gateway.listLoads, 1);
    expect(gateway.createBodies, hasLength(1));
    expect(find.text('문서 정리'), findsOneWidget);
    expect(find.text('확장자 [.pdf] · 버전 1'), findsOneWidget);
  });

  test('upsertRule replaces existing rule and keeps priority order', () {
    final result = upsertRule([
      _rule('later', 'Later', priority: 100, createdAt: '2026-07-14T00:02:00Z'),
      _rule('target', 'Old', priority: 100, createdAt: '2026-07-14T00:01:00Z'),
    ], _rule('target', 'Updated', version: 2, priority: 50));

    expect(result.map((rule) => rule['id']), ['target', 'later']);
    expect(result.first['name'], 'Updated');
    expect(result.first['version'], 2);
  });
}

Future<void> _pumpRules(WidgetTester tester, _FakeRuleGateway gateway) async {
  await tester.pumpWidget(
    ProviderScope(
      child: MaterialApp(
        home: RulesPage(roomId: 'room-1', gateway: gateway),
      ),
    ),
  );
  await tester.pumpAndSettle();
}

Map<String, dynamic> _rule(
  String id,
  String name, {
  bool enabled = true,
  int version = 1,
  int priority = 100,
  String createdAt = '2026-07-14T00:00:00.000Z',
  String extension = '.pdf',
}) => {
  'id': id,
  'roomId': 'room-1',
  'name': name,
  'definition': {
    'match': 'ALL',
    'conditions': [
      {
        'field': 'extension',
        'operator': 'IN',
        'value': [extension],
      },
    ],
    'action': {'type': 'MOVE', 'destinationTemplate': 'Archive'},
  },
  'priority': priority,
  'enabled': enabled,
  'version': version,
  'createdAt': createdAt,
  'updatedAt': createdAt,
};

class _FakeRuleGateway implements RuleGateway {
  _FakeRuleGateway(this.rules);

  final List<Map<String, dynamic>> rules;
  final List<Map<String, dynamic>> createBodies = [];
  final List<Map<String, dynamic>> updateBodies = [];
  int listLoads = 0;

  @override
  Future<List<Map<String, dynamic>>> listRules(String roomId) async {
    listLoads += 1;
    return [for (final rule in rules) Map<String, dynamic>.from(rule)];
  }

  @override
  Future<Map<String, dynamic>> createRule(
    String roomId,
    Map<String, dynamic> body,
  ) async {
    createBodies.add(Map<String, dynamic>.from(body));
    final created = {
      ..._rule(
        'created-${createBodies.length}',
        body['name'] as String,
        extension:
            (((body['definition'] as Map)['conditions'] as List).first
                    as Map)['value'][0]
                as String,
      ),
      ...body,
    };
    rules.add(created);
    return created;
  }

  @override
  Future<Map<String, dynamic>> updateRule(
    String ruleId,
    Map<String, dynamic> body,
  ) async {
    updateBodies.add({'ruleId': ruleId, ...body});
    final index = rules.indexWhere((rule) => rule['id'] == ruleId);
    final current = rules[index];
    final updated = {
      ...current,
      ...body,
      'id': ruleId,
      'version': (body['version'] as int) + 1,
    };
    rules[index] = updated;
    return updated;
  }
}
