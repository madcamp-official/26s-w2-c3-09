import 'dart:collection';
import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/features/games/cage_escape_game_page.dart';
import 'package:mousekeeper/features/games/maze_game_page.dart';
import 'package:mousekeeper/features/games/mini_game_hub_page.dart';
import 'package:mousekeeper/features/games/whack_game_page.dart';

void main() {
  test('generated maze connects every cell to the start', () {
    final board = MazeBoard.generate(size: 10, random: Random(7));
    final visited = <Point<int>>{const Point(0, 0)};
    final pending = Queue<Point<int>>()..add(const Point(0, 0));

    while (pending.isNotEmpty) {
      final current = pending.removeFirst();
      for (final direction in MazeDirection.values) {
        if (!board.canMove(current, direction)) continue;
        final next = board.move(current, direction);
        if (visited.add(next)) pending.add(next);
      }
    }

    expect(visited, hasLength(100));
    expect(visited, contains(const Point(9, 9)));
  });

  testWidgets('game hub routes to all three playable games', (tester) async {
    await tester.pumpWidget(const MaterialApp(home: MiniGameHubPage()));

    expect(find.text('미로 찾기'), findsOneWidget);
    expect(find.text('치즈 잡기'), findsOneWidget);
    expect(find.text('케이지 탈출'), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('game-card-maze')));
    await tester.pumpAndSettle();
    expect(find.byType(MazeGamePage), findsOneWidget);
    await tester.pageBack();
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const ValueKey('game-card-whack')));
    await tester.pumpAndSettle();
    expect(find.byType(WhackGamePage), findsOneWidget);
    await tester.pageBack();
    await tester.pumpAndSettle();

    final cageCard = find.byKey(const ValueKey('game-card-cage'));
    await tester.scrollUntilVisible(cageCard, 180);
    await tester.pumpAndSettle();
    await tester.tap(cageCard);
    // The escape game owns a continuous physics ticker, so settle would never
    // become idle. Advance just long enough for the route animation instead.
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.byType(CageEscapeGamePage), findsOneWidget);
    expect(find.text('JUMP'), findsOneWidget);
    await tester.pumpWidget(const SizedBox());
  });
}
