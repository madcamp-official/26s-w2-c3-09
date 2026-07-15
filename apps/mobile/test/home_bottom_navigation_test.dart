import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/features/navigation/home_bottom_navigation.dart';

void main() {
  testWidgets('four pixel menu actions are interactive and evenly available', (
    tester,
  ) async {
    HomeMenuAction? selected;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Center(
            child: HomeBottomNavigation(
              selected: null,
              onSelected: (action) => selected = action,
            ),
          ),
        ),
      ),
    );

    for (final action in HomeMenuAction.values) {
      expect(find.byKey(ValueKey('home-menu-${action.name}')), findsOneWidget);
    }

    await tester.tap(find.byKey(const ValueKey('home-menu-feeding')));
    expect(selected, HomeMenuAction.feeding);
  });
}
