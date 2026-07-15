import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/features/feeding/feeding_interaction.dart';

void main() {
  test(
    'cheese follows the pointer and feeds only inside the mouse collider',
    () async {
      final controller = FeedingController(animationDuration: Duration.zero);
      addTearDown(controller.dispose);

      controller.activate();
      expect(controller.state.isActive, isTrue);

      final missed = await controller.dragCheese(
        const Offset(20, 20),
        const Rect.fromLTWH(100, 100, 100, 100),
      );
      expect(missed, isFalse);
      expect(controller.state.cheesePosition, const Offset(20, 20));

      final fed = await controller.dragCheese(
        const Offset(150, 150),
        const Rect.fromLTWH(100, 100, 100, 100),
      );
      expect(fed, isTrue);
      expect(controller.state.isActive, isFalse);
      expect(controller.state.isFeeding, isFalse);
      expect(controller.state.cheesePosition, isNull);
    },
  );

  testWidgets('feeding layer exposes guidance before the first drag', (
    tester,
  ) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(
          body: FeedingGestureLayer(
            state: FeedingState(isActive: true),
            mouseCollider: _noCollider,
            onPointer: _ignorePointer,
          ),
        ),
      ),
    );

    expect(find.text('화면을 누르고 치즈를 생쥐에게 드래그하세요!'), findsOneWidget);
  });
}

Rect? _noCollider() => null;
void _ignorePointer(Offset _) {}
