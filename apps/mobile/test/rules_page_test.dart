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

  testWidgets('creating an expanded DSL rule can use size and trash action', (
    tester,
  ) async {
    final gateway = _FakeRuleGateway([]);

    await _pumpRules(tester, gateway);
    await tester.tap(find.text('규칙 추가'));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const ValueKey('rule-name-field')),
      '작은 파일 휴지통',
    );

    await tester.tap(find.byKey(const ValueKey('rule-condition-field')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('파일 크기 이하').last);
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const ValueKey('rule-condition-value-field')),
      '1048576',
    );

    await tester.tap(find.byKey(const ValueKey('rule-action-field')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('휴지통').last);
    await tester.pumpAndSettle();

    expect(find.byKey(const ValueKey('rule-destination-field')), findsNothing);
    await tester.tap(find.text('저장'));
    await tester.pumpAndSettle();

    expect(gateway.listLoads, 1);
    expect(gateway.createBodies.single['definition'], {
      'match': 'ALL',
      'conditions': [
        {'field': 'sizeBytes', 'operator': 'LTE', 'value': 1048576},
      ],
      'action': {'type': 'TRASH'},
    });
    expect(find.text('작은 파일 휴지통'), findsOneWidget);
    expect(find.text('크기 LTE 1048576 bytes · 버전 1'), findsOneWidget);
  });

  testWidgets('AI rule draft UNCONFIGURED does not create a fake rule', (
    tester,
  ) async {
    final gateway = _FakeRuleGateway([]);
    gateway.nextDraftResult = {
      'status': 'UNCONFIGURED',
      'code': 'AI_PROVIDER_UNCONFIGURED',
    };

    await _pumpRules(tester, gateway);
    await tester.enterText(
      find.byKey(const ValueKey('rule-draft-instruction-field')),
      'Move old PDFs',
    );
    await tester.tap(find.byKey(const ValueKey('rule-draft-submit')));
    await tester.pumpAndSettle();

    expect(gateway.createDraftInstructions, ['Move old PDFs']);
    expect(gateway.listLoads, 1);
    expect(gateway.createBodies, isEmpty);
    expect(
      find.text('AI rule draft provider is UNCONFIGURED.'),
      findsOneWidget,
    );
    expect(find.byKey(const ValueKey('rule-draft-confirm')), findsNothing);
  });

  testWidgets('READY AI rule draft confirms into a rule without full reload', (
    tester,
  ) async {
    final gateway = _FakeRuleGateway([]);
    gateway.nextDraftResult = {
      'status': 'READY',
      'kind': 'RULE_DRAFT',
      'draft': {
        'id': 'draft-1',
        'roomId': 'room-1',
        'name': 'Old PDFs',
        'definition': {
          'match': 'ALL',
          'conditions': [
            {
              'field': 'extension',
              'operator': 'IN',
              'value': ['.pdf'],
            },
          ],
          'action': {'type': 'MOVE', 'destinationTemplate': 'Archive'},
        },
        'explanation': 'Move old PDFs after explicit approval.',
        'ambiguities': [],
        'status': 'DRAFT',
        'expiresAt': '2026-07-14T00:00:00.000Z',
        'ruleId': null,
      },
    };

    await _pumpRules(tester, gateway);
    await tester.enterText(
      find.byKey(const ValueKey('rule-draft-instruction-field')),
      'Move old PDFs',
    );
    await tester.tap(find.byKey(const ValueKey('rule-draft-submit')));
    await tester.pumpAndSettle();
    expect(find.text('Old PDFs'), findsOneWidget);
    expect(find.text('Move old PDFs after explicit approval.'), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('rule-draft-confirm')));
    await tester.pumpAndSettle();

    expect(gateway.confirmedDraftIds, ['draft-1']);
    expect(gateway.listLoads, 1);
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

  test(
    'ruleConditionBody and ruleActionBody build expanded contract payloads',
    () {
      expect(
        ruleConditionBody(
          conditionId: 'fileKind',
          rawValue: '',
          fileKindValue: 'DIRECTORY',
        ),
        {'field': 'fileKind', 'operator': 'EQ', 'value': 'DIRECTORY'},
      );
      expect(
        ruleConditionBody(
          conditionId: 'createdAgeDaysGt',
          rawValue: '7',
          fileKindValue: 'FILE',
        ),
        {'field': 'createdAgeDays', 'operator': 'GT', 'value': 7},
      );
      expect(
        ruleConditionBody(
          conditionId: 'sizeBytesGte',
          rawValue: 'not-a-number',
          fileKindValue: 'FILE',
        ),
        isNull,
      );
      expect(
        ruleActionBody(actionType: 'CREATE_DIR', rawPath: 'Archive/Reports'),
        {'type': 'CREATE_DIR', 'relativePath': 'Archive/Reports'},
      );
      expect(ruleActionBody(actionType: 'MOVE', rawPath: ''), isNull);
    },
  );
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
  final List<String> createDraftInstructions = [];
  final List<String> confirmedDraftIds = [];
  final List<String> rejectedDraftIds = [];
  Map<String, dynamic> nextDraftResult = {
    'status': 'UNCONFIGURED',
    'code': 'AI_PROVIDER_UNCONFIGURED',
  };
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
      ..._rule('created-${createBodies.length}', body['name'] as String),
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

  @override
  Future<Map<String, dynamic>> createRuleDraft(
    String roomId,
    String instruction,
  ) async {
    createDraftInstructions.add(instruction);
    return Map<String, dynamic>.from(nextDraftResult);
  }

  @override
  Future<Map<String, dynamic>> confirmRuleDraft(
    String draftId,
    String idempotencyKey,
  ) async {
    confirmedDraftIds.add(draftId);
    final draft = Map<String, dynamic>.from(nextDraftResult['draft'] as Map);
    final rule = {
      ..._rule('rule-from-$draftId', draft['name'] as String),
      'definition': draft['definition'],
      'priority': 100,
      'enabled': true,
      'version': 1,
    };
    rules.add(rule);
    return {
      'draft': {...draft, 'status': 'MATERIALIZED', 'ruleId': rule['id']},
      'rule': rule,
    };
  }

  @override
  Future<Map<String, dynamic>> rejectRuleDraft(String draftId) async {
    rejectedDraftIds.add(draftId);
    final draftRaw = nextDraftResult['draft'];
    final draft = draftRaw is Map
        ? Map<String, dynamic>.from(draftRaw)
        : <String, dynamic>{'id': draftId};
    return {
      'draft': {...draft, 'status': 'REJECTED', 'ruleId': null},
    };
  }
}
