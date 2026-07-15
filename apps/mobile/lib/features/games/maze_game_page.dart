import 'dart:async';
import 'dart:math';

import 'package:flutter/material.dart';

import 'pixel_game_widgets.dart';

enum MazeDirection { up, right, down, left }

extension on MazeDirection {
  Point<int> get delta => switch (this) {
    MazeDirection.up => const Point(0, -1),
    MazeDirection.right => const Point(1, 0),
    MazeDirection.down => const Point(0, 1),
    MazeDirection.left => const Point(-1, 0),
  };

  MazeDirection get opposite => switch (this) {
    MazeDirection.up => MazeDirection.down,
    MazeDirection.right => MazeDirection.left,
    MazeDirection.down => MazeDirection.up,
    MazeDirection.left => MazeDirection.right,
  };

  int get bit => 1 << index;
}

class MazeBoard {
  MazeBoard._(this.size, this._openings);

  factory MazeBoard.generate({int size = 10, Random? random}) {
    assert(size >= 4);
    final rng = random ?? Random();
    final openings = List<int>.filled(size * size, 0);
    final visited = List<bool>.filled(size * size, false);
    final stack = <Point<int>>[const Point(0, 0)];
    visited[0] = true;

    while (stack.isNotEmpty) {
      final current = stack.last;
      final directions = MazeDirection.values.toList()..shuffle(rng);
      Point<int>? next;
      MazeDirection? chosen;
      for (final direction in directions) {
        final candidate = Point(
          current.x + direction.delta.x,
          current.y + direction.delta.y,
        );
        if (candidate.x < 0 ||
            candidate.y < 0 ||
            candidate.x >= size ||
            candidate.y >= size ||
            visited[candidate.y * size + candidate.x]) {
          continue;
        }
        next = candidate;
        chosen = direction;
        break;
      }
      if (next == null || chosen == null) {
        stack.removeLast();
        continue;
      }
      openings[current.y * size + current.x] |= chosen.bit;
      openings[next.y * size + next.x] |= chosen.opposite.bit;
      visited[next.y * size + next.x] = true;
      stack.add(next);
    }
    return MazeBoard._(size, openings);
  }

  final int size;
  final List<int> _openings;

  bool canMove(Point<int> position, MazeDirection direction) =>
      _openings[position.y * size + position.x] & direction.bit != 0;

  Point<int> move(Point<int> position, MazeDirection direction) {
    if (!canMove(position, direction)) return position;
    return Point(
      position.x + direction.delta.x,
      position.y + direction.delta.y,
    );
  }
}

class MazeGamePage extends StatefulWidget {
  const MazeGamePage({super.key});

  @override
  State<MazeGamePage> createState() => _MazeGamePageState();
}

class _MazeGamePageState extends State<MazeGamePage> {
  late MazeBoard _board;
  Point<int> _player = const Point(0, 0);
  late DateTime _startedAt;
  Duration _elapsed = Duration.zero;
  Timer? _timer;
  bool _won = false;

  @override
  void initState() {
    super.initState();
    _restart();
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  void _restart() {
    _timer?.cancel();
    _board = MazeBoard.generate();
    _player = const Point(0, 0);
    _startedAt = DateTime.now();
    _elapsed = Duration.zero;
    _won = false;
    _timer = Timer.periodic(const Duration(milliseconds: 100), (_) {
      if (mounted) {
        setState(() => _elapsed = DateTime.now().difference(_startedAt));
      }
    });
    if (mounted) setState(() {});
  }

  void _move(MazeDirection direction) {
    if (_won) return;
    final next = _board.move(_player, direction);
    if (next == _player) return;
    setState(() => _player = next);
    if (_player == Point(_board.size - 1, _board.size - 1)) {
      _complete();
    }
  }

  Future<void> _complete() async {
    if (_won) return;
    _won = true;
    _timer?.cancel();
    setState(() => _elapsed = DateTime.now().difference(_startedAt));
    await showDialog<void>(
      context: context,
      barrierDismissible: false,
      builder: (context) => AlertDialog(
        title: const Text('치즈를 찾았어요!'),
        content: Text('성공 시간: ${_formatTime(_elapsed)}'),
        actions: [
          TextButton(
            onPressed: () {
              Navigator.of(context).pop();
              _restart();
            },
            child: const Text('새 미로'),
          ),
          FilledButton(
            onPressed: () {
              Navigator.of(context).pop();
              Navigator.of(this.context).pop();
            },
            child: const Text('허브로'),
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) => Scaffold(
    backgroundColor: pixelGameSky,
    appBar: AppBar(
      title: const Text('미로 찾기'),
      actions: [
        Center(child: Text(_formatTime(_elapsed))),
        IconButton(onPressed: _restart, icon: const Icon(Icons.refresh)),
      ],
    ),
    body: SafeArea(
      top: false,
      child: Padding(
        padding: const EdgeInsets.fromLTRB(16, 16, 16, 20),
        child: Column(
          children: [
            Expanded(
              child: Center(
                child: PixelGamePanel(
                  padding: const EdgeInsets.all(8),
                  child: AspectRatio(
                    aspectRatio: 1,
                    child: GestureDetector(
                      onPanEnd: (details) {
                        final velocity = details.velocity.pixelsPerSecond;
                        if (velocity.distance < 80) return;
                        _move(
                          velocity.dx.abs() > velocity.dy.abs()
                              ? velocity.dx > 0
                                    ? MazeDirection.right
                                    : MazeDirection.left
                              : velocity.dy > 0
                              ? MazeDirection.down
                              : MazeDirection.up,
                        );
                      },
                      child: CustomPaint(
                        painter: _MazePainter(board: _board, player: _player),
                      ),
                    ),
                  ),
                ),
              ),
            ),
            const SizedBox(height: 18),
            _MazeDPad(onMove: _move),
          ],
        ),
      ),
    ),
  );
}

String _formatTime(Duration duration) {
  final minutes = duration.inMinutes.toString().padLeft(2, '0');
  final seconds = (duration.inSeconds % 60).toString().padLeft(2, '0');
  final tenths = ((duration.inMilliseconds % 1000) ~/ 100);
  return '$minutes:$seconds.$tenths';
}

class _MazeDPad extends StatelessWidget {
  const _MazeDPad({required this.onMove});

  final ValueChanged<MazeDirection> onMove;

  @override
  Widget build(BuildContext context) => Column(
    mainAxisSize: MainAxisSize.min,
    children: [
      PixelControlButton(
        tooltip: '위',
        onPressed: () => onMove(MazeDirection.up),
        child: const Icon(Icons.keyboard_arrow_up),
      ),
      Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          PixelControlButton(
            tooltip: '왼쪽',
            onPressed: () => onMove(MazeDirection.left),
            child: const Icon(Icons.keyboard_arrow_left),
          ),
          const SizedBox(width: 58),
          PixelControlButton(
            tooltip: '오른쪽',
            onPressed: () => onMove(MazeDirection.right),
            child: const Icon(Icons.keyboard_arrow_right),
          ),
        ],
      ),
      PixelControlButton(
        tooltip: '아래',
        onPressed: () => onMove(MazeDirection.down),
        child: const Icon(Icons.keyboard_arrow_down),
      ),
    ],
  );
}

class _MazePainter extends CustomPainter {
  const _MazePainter({required this.board, required this.player});

  final MazeBoard board;
  final Point<int> player;

  @override
  void paint(Canvas canvas, Size size) {
    canvas.drawRect(
      Offset.zero & size,
      Paint()..color = const Color(0xFFFFF4D6),
    );
    final cell = size.shortestSide / board.size;
    final origin = Offset((size.width - cell * board.size) / 2, 0);
    final wall = Paint()
      ..color = pixelGameInk
      ..strokeWidth = 2
      ..style = PaintingStyle.stroke
      ..isAntiAlias = false;
    for (var y = 0; y < board.size; y++) {
      for (var x = 0; x < board.size; x++) {
        final position = Point(x, y);
        final left = origin.dx + x * cell;
        final top = y * cell;
        if (!board.canMove(position, MazeDirection.up)) {
          canvas.drawLine(Offset(left, top), Offset(left + cell, top), wall);
        }
        if (!board.canMove(position, MazeDirection.left)) {
          canvas.drawLine(Offset(left, top), Offset(left, top + cell), wall);
        }
        if (y == board.size - 1) {
          canvas.drawLine(
            Offset(left, top + cell),
            Offset(left + cell, top + cell),
            wall,
          );
        }
        if (x == board.size - 1) {
          canvas.drawLine(
            Offset(left + cell, top),
            Offset(left + cell, top + cell),
            wall,
          );
        }
      }
    }

    final goalCenter = Offset(
      origin.dx + (board.size - 0.5) * cell,
      (board.size - 0.5) * cell,
    );
    final cheese = Paint()
      ..color = pixelGameAccent
      ..isAntiAlias = false;
    canvas.drawRect(
      Rect.fromCenter(
        center: goalCenter,
        width: cell * 0.55,
        height: cell * 0.55,
      ),
      cheese,
    );
    final playerCenter = Offset(
      origin.dx + (player.x + 0.5) * cell,
      (player.y + 0.5) * cell,
    );
    _paintMouseFace(canvas, playerCenter, cell * 0.82);
  }

  void _paintMouseFace(Canvas canvas, Offset center, double size) {
    // Primitive, non-antialiased shapes keep the marker readable as a mouse
    // face even when a maze cell is only a few dozen physical pixels wide.
    final fur = Paint()
      ..color = const Color(0xFFFFE2C2)
      ..isAntiAlias = false;
    final innerEar = Paint()
      ..color = const Color(0xFFE78596)
      ..isAntiAlias = false;
    final ink = Paint()
      ..color = pixelGameInk
      ..isAntiAlias = false;
    final earSize = size * 0.32;
    final head = Rect.fromCenter(
      center: center + Offset(0, size * 0.06),
      width: size * 0.7,
      height: size * 0.62,
    );
    final leftEar = Rect.fromCenter(
      center: center + Offset(-size * 0.27, -size * 0.24),
      width: earSize,
      height: earSize,
    );
    final rightEar = Rect.fromCenter(
      center: center + Offset(size * 0.27, -size * 0.24),
      width: earSize,
      height: earSize,
    );
    canvas
      ..drawRect(leftEar, fur)
      ..drawRect(rightEar, fur)
      ..drawRect(leftEar.deflate(size * 0.08), innerEar)
      ..drawRect(rightEar.deflate(size * 0.08), innerEar)
      ..drawRect(head, fur);

    final eyeSize = max(2.0, size * 0.09);
    canvas
      ..drawRect(
        Rect.fromCenter(
          center: center + Offset(-size * 0.17, 0),
          width: eyeSize,
          height: eyeSize,
        ),
        ink,
      )
      ..drawRect(
        Rect.fromCenter(
          center: center + Offset(size * 0.17, 0),
          width: eyeSize,
          height: eyeSize,
        ),
        ink,
      )
      ..drawRect(
        Rect.fromCenter(
          center: center + Offset(0, size * 0.18),
          width: eyeSize,
          height: eyeSize * 0.75,
        ),
        innerEar,
      );
  }

  @override
  bool shouldRepaint(covariant _MazePainter oldDelegate) =>
      oldDelegate.board != board || oldDelegate.player != player;
}
