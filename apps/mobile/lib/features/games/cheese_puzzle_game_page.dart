import 'dart:math' as math;

import 'package:flutter/material.dart';

class CheesePuzzleGamePage extends StatefulWidget {
  const CheesePuzzleGamePage({super.key});

  @override
  State<CheesePuzzleGamePage> createState() => _CheesePuzzleGamePageState();
}

class _Stage {
  _Stage({
    required this.rows,
    required this.start,
    required this.cheese,
    required this.walls,
    required this.boxes,
    required this.turns,
  });
  final List<String> rows;
  final math.Point<int> start;
  final math.Point<int> cheese;
  final Set<math.Point<int>> walls;
  final Set<math.Point<int>> boxes;
  final int turns;
}

final _stages = <_Stage>[
  _Stage(
    rows: [
      '########',
      '#......#',
      '#.##...#',
      '#......#',
      '#...##.#',
      '#......#',
      '########',
    ],
    start: math.Point(1, 1),
    cheese: math.Point(6, 5),
    walls: {
      math.Point(2, 2),
      math.Point(3, 2),
      math.Point(4, 4),
      math.Point(5, 4),
    },
    boxes: {math.Point(3, 3)},
    turns: 18,
  ),
  _Stage(
    rows: [
      '#########',
      '#.......#',
      '#.###...#',
      '#...#.#.#',
      '#.#...#.#',
      '#...###.#',
      '#.......#',
      '#########',
    ],
    start: math.Point(1, 1),
    cheese: math.Point(7, 6),
    walls: {
      math.Point(2, 2),
      math.Point(3, 2),
      math.Point(4, 2),
      math.Point(4, 3),
      math.Point(2, 4),
      math.Point(6, 3),
      math.Point(6, 4),
      math.Point(3, 5),
      math.Point(4, 5),
      math.Point(5, 5),
    },
    boxes: {math.Point(5, 3), math.Point(2, 6)},
    turns: 24,
  ),
  _Stage(
    rows: [
      '##########',
      '#........#',
      '#.####...#',
      '#....#.#.#',
      '###..#.#.#',
      '#....#...#',
      '#.######.#',
      '#........#',
      '##########',
    ],
    start: math.Point(1, 1),
    cheese: math.Point(8, 7),
    walls: {
      math.Point(2, 2),
      math.Point(3, 2),
      math.Point(4, 2),
      math.Point(5, 2),
      math.Point(5, 3),
      math.Point(7, 3),
      math.Point(0, 4),
      math.Point(1, 4),
      math.Point(2, 4),
      math.Point(5, 4),
      math.Point(7, 4),
      math.Point(5, 5),
      math.Point(2, 6),
      math.Point(3, 6),
      math.Point(4, 6),
      math.Point(5, 6),
    },
    boxes: {math.Point(4, 3), math.Point(3, 5), math.Point(6, 7)},
    turns: 32,
  ),
];

class _CheesePuzzleGamePageState extends State<CheesePuzzleGamePage> {
  int _stage = 0;
  late math.Point<int> _player;
  late int _turns;
  bool _won = false;
  bool _failed = false;

  _Stage get _current => _stages[_stage];

  @override
  void initState() {
    super.initState();
    _reset();
  }

  void _reset() => setState(() {
    _player = _current.start;
    _turns = _current.turns;
    _won = false;
    _failed = false;
  });

  void _move(math.Point<int> delta) {
    if (_won || _failed) return;
    final next = math.Point(_player.x + delta.x, _player.y + delta.y);
    if (next.x < 0 ||
        next.y < 0 ||
        next.y >= _current.rows.length ||
        next.x >= _current.rows[next.y].length ||
        _current.rows[next.y][next.x] == '#' ||
        _current.walls.contains(next) ||
        _current.boxes.contains(next))
      return;
    setState(() {
      _player = next;
      _turns--;
      if (_player == _current.cheese)
        _won = true;
      else if (_turns <= 0)
        _failed = true;
    });
  }

  void _next() {
    if (_stage == _stages.length - 1) {
      _reset();
      return;
    }
    setState(() {
      _stage++;
    });
    _reset();
  }

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: AppBar(title: const Text('치즈 탈출 퍼즐')),
    body: SafeArea(
      child: Column(
        children: [
          Padding(
            padding: const EdgeInsets.all(16),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                Text(
                  'STAGE ${_stage + 1} / ${_stages.length}',
                  style: const TextStyle(fontWeight: FontWeight.bold),
                ),
                Text(
                  '남은 턴 $_turns',
                  style: TextStyle(
                    color: _turns <= 5 ? Colors.red : null,
                    fontWeight: FontWeight.bold,
                  ),
                ),
              ],
            ),
          ),
          Expanded(
            child: Center(
              child: AspectRatio(
                aspectRatio: 1.15,
                child: LayoutBuilder(
                  builder: (context, c) {
                    final cell = math.min(
                      c.maxWidth / _current.rows[0].length,
                      c.maxHeight / _current.rows.length,
                    );
                    final ox =
                        (c.maxWidth - cell * _current.rows[0].length) / 2;
                    final oy = (c.maxHeight - cell * _current.rows.length) / 2;
                    return Stack(
                      children: [
                        CustomPaint(
                          size: c.biggest,
                          painter: _BoardPainter(
                            stage: _current,
                            player: _player,
                          ),
                        ),
                        AnimatedPositioned(
                          duration: const Duration(milliseconds: 120),
                          left: ox + _player.x * cell,
                          top: oy + _player.y * cell,
                          width: cell,
                          height: cell,
                          child: Image.asset(
                            'assets/character/cheese_eating.gif',
                            fit: BoxFit.contain,
                            filterQuality: FilterQuality.none,
                          ),
                        ),
                      ],
                    );
                  },
                ),
              ),
            ),
          ),
          if (_won || _failed)
            Padding(
              padding: const EdgeInsets.all(8),
              child: Text(_won ? '클리어!' : '실패!'),
            ),
          _DPad(onMove: _move),
          const SizedBox(height: 16),
        ],
      ),
    ),
  );
}

class _DPad extends StatelessWidget {
  const _DPad({required this.onMove});
  final ValueChanged<math.Point<int>> onMove;
  @override
  Widget build(BuildContext context) => Column(
    children: [
      IconButton(
        onPressed: () => onMove(const math.Point(0, -1)),
        icon: const Icon(Icons.keyboard_arrow_up, size: 36),
      ),
      Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          IconButton(
            onPressed: () => onMove(const math.Point(-1, 0)),
            icon: const Icon(Icons.keyboard_arrow_left, size: 36),
          ),
          const SizedBox(width: 40),
          IconButton(
            onPressed: () => onMove(const math.Point(1, 0)),
            icon: const Icon(Icons.keyboard_arrow_right, size: 36),
          ),
        ],
      ),
      IconButton(
        onPressed: () => onMove(const math.Point(0, 1)),
        icon: const Icon(Icons.keyboard_arrow_down, size: 36),
      ),
    ],
  );
}

class _BoardPainter extends CustomPainter {
  const _BoardPainter({required this.stage, required this.player});
  final _Stage stage;
  final math.Point<int> player;
  @override
  void paint(Canvas canvas, Size size) {
    final cell = math.min(
      size.width / stage.rows[0].length,
      size.height / stage.rows.length,
    );
    final ox = (size.width - cell * stage.rows[0].length) / 2;
    final oy = (size.height - cell * stage.rows.length) / 2;
    final p = Paint();
    for (var y = 0; y < stage.rows.length; y++)
      for (var x = 0; x < stage.rows[y].length; x++) {
        final r = Rect.fromLTWH(
          ox + x * cell,
          oy + y * cell,
          cell - 1,
          cell - 1,
        );
        p.color =
            stage.rows[y][x] == '#' || stage.walls.contains(math.Point(x, y))
            ? const Color(0xFF4B3A50)
            : const Color(0xFFFFF1D6);
        canvas.drawRect(r, p);
        final point = math.Point(x, y);
        if (point == stage.cheese) {
          p.color = const Color(0xFFFFC857);
          canvas.drawCircle(r.center, cell * .28, p);
        }
        if (stage.boxes.contains(point)) {
          p.color = const Color(0xFF9A5B27);
          canvas.drawRect(r.deflate(cell * .18), p);
        }
      }
  }

  @override
  bool shouldRepaint(covariant _BoardPainter old) =>
      old.player != player || old.stage != stage;
}
