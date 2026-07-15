import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/features/navigation/home_bottom_navigation.dart';

void main() {
  testWidgets('four provided menu icons are interactive and evenly available', (
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

    const expectedAssets = <HomeMenuAction, String>{
      HomeMenuAction.files: 'assets/images/home_navigation/folder.png',
      HomeMenuAction.games: 'assets/images/home_navigation/console.png',
      HomeMenuAction.feeding: 'assets/images/home_navigation/cheese.png',
      HomeMenuAction.wardrobe: 'assets/images/home_navigation/hanger.png',
    };
    for (final action in HomeMenuAction.values) {
      expect(find.byKey(ValueKey('home-menu-${action.name}')), findsOneWidget);
      final image = tester.widget<Image>(
        find.byKey(ValueKey('home-menu-icon-${action.name}')),
      );
      expect(image.image, isA<AssetImage>());
      expect((image.image as AssetImage).assetName, expectedAssets[action]);
      await tester.tap(find.byKey(ValueKey('home-menu-${action.name}')));
      expect(selected, action);
    }
  });
}
