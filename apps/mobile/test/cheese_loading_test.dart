import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/core/widgets/cheese_loading.dart';

void main() {
  testWidgets('loading overlay dims, blocks, and shows the cheese message', (
    tester,
  ) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(
          body: CheeseLoadingOverlay(
            loading: true,
            progress: 0.5,
            message: '응답을 기다리는 중입니다',
            child: Text('기존 화면'),
          ),
        ),
      ),
    );

    expect(find.text('기존 화면'), findsOneWidget);
    expect(find.byKey(const ValueKey('cheese-loading-dim')), findsOneWidget);
    expect(
      find.byKey(const ValueKey('cheese-loading-blocker')),
      findsOneWidget,
    );
    expect(find.byType(CheeseLoadingIndicator), findsOneWidget);
    expect(find.text('응답을 기다리는 중입니다'), findsOneWidget);
  });

  testWidgets('loading overlay leaves the child untouched when idle', (
    tester,
  ) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(
          body: CheeseLoadingOverlay(loading: false, child: Text('준비 완료')),
        ),
      ),
    );

    expect(find.text('준비 완료'), findsOneWidget);
    expect(find.byKey(const ValueKey('cheese-loading-dim')), findsNothing);
    expect(find.byType(CheeseLoadingIndicator), findsNothing);
  });
}
