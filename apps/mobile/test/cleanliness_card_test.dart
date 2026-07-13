import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/features/rooms/room_page.dart';

void main() {
  testWidgets('공식 버전과 감점 상세 및 계산 시각을 그대로 표시한다', (tester) async {
    final snapshot = _loadContractSnapshotFixture();
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SingleChildScrollView(
            child: CleanlinessCard(snapshot: snapshot),
          ),
        ),
      ),
    );

    expect(find.text('53'), findsOneWidget);
    expect(
      find.textContaining(supportedCleanlinessFormulaVersion),
      findsOneWidget,
    );
    expect(find.textContaining('마지막 계산'), findsOneWidget);
    expect(find.textContaining('UNORGANIZED_FILES'), findsOneWidget);
    expect(find.textContaining('2개 · -47점'), findsOneWidget);
  });

  testWidgets('알 수 없는 공식 버전은 점수와 조용히 혼합하지 않는다', (tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(
          body: CleanlinessCard(
            snapshot: {
              'score': 99,
              'formulaVersion': 'mousekeeper-cleanliness-v2',
              'calculatedAt': '2026-07-13T10:20:00.000Z',
              'metrics': {'deductions': <Object>[]},
            },
          ),
        ),
      ),
    );

    expect(find.text('청결도 공식 업데이트 필요'), findsOneWidget);
    expect(find.textContaining('mousekeeper-cleanliness-v2'), findsOneWidget);
    expect(find.text('99'), findsNothing);
  });
}

Map<String, dynamic> _loadContractSnapshotFixture() {
  for (final path in const [
    '../../packages/contracts/fixtures/room-snapshot-v1.json',
    'packages/contracts/fixtures/room-snapshot-v1.json',
  ]) {
    final file = File(path);
    if (file.existsSync()) {
      return Map<String, dynamic>.from(
        jsonDecode(file.readAsStringSync()) as Map,
      );
    }
  }
  throw StateError('ROOM_SNAPSHOT_CONTRACT_FIXTURE_NOT_FOUND');
}
