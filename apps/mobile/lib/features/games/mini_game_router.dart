import 'package:flutter/material.dart';

import 'cage_escape_game_page.dart';
import 'maze_game_page.dart';
import 'whack_game_page.dart';

enum MiniGameKind { maze, whack, cageEscape }

Route<void> miniGameRoute(MiniGameKind game) => MaterialPageRoute<void>(
  builder: (_) => switch (game) {
    MiniGameKind.maze => const MazeGamePage(),
    MiniGameKind.whack => const WhackGamePage(),
    MiniGameKind.cageEscape => const CageEscapeGamePage(),
  },
);
