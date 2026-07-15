import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';

import 'pixel_game_widgets.dart';

class CageEscapeGamePage extends StatefulWidget {
  const CageEscapeGamePage({super.key});

  @override
  State<CageEscapeGamePage> createState() => _CageEscapeGamePageState();
}

class _CageEscapeGamePageState extends State<CageEscapeGamePage>
    with SingleTickerProviderStateMixin {
  static const _worldSize = Size(360, 600);
  static const _playerSize = Size(28, 32);
  static const _gravity = 900.0;
  static const _moveSpeed = 155.0;
  static const _jumpSpeed = -430.0;
  static const _start = Offset(22, 520);
  static const _exit = Rect.fromLTWH(298, 96, 42, 58);
  static const _platforms = <Rect>[
    Rect.fromLTWH(0, 560, 360, 40),
    Rect.fromLTWH(55, 486, 98, 14),
    Rect.fromLTWH(190, 414, 104, 14),
    Rect.fromLTWH(72, 340, 106, 14),
    Rect.fromLTWH(205, 266, 100, 14),
    Rect.fromLTWH(78, 198, 105, 14),
    Rect.fromLTWH(244, 154, 100, 14),
  ];
  static const _hazards = <Rect>[
    Rect.fromLTWH(144, 542, 42, 18),
    Rect.fromLTWH(239, 396, 32, 18),
    Rect.fromLTWH(112, 322, 30, 18),
    Rect.fromLTWH(240, 248, 30, 18),
  ];

  late final Ticker _ticker;
  Duration? _lastTick;
  late DateTime _startedAt;
  Offset _position = _start;
  Offset _velocity = Offset.zero;
  bool _moveLeft = false;
  bool _moveRight = false;
  bool _grounded = false;
  bool _cleared = false;
  int _respawns = 0;

  @override
  void initState() {
    super.initState();
    _startedAt = DateTime.now();
    _ticker = createTicker(_tick)..start();
  }

  @override
  void dispose() {
    _ticker.dispose();
    super.dispose();
  }

  Rect get _playerRect => _position & _playerSize;

  void _tick(Duration elapsed) {
    final previous = _lastTick;
    _lastTick = elapsed;
    if (previous == null || _cleared || !mounted) return;
    final dt = math.min(
      0.033,
      (elapsed - previous).inMicroseconds / Duration.microsecondsPerSecond,
    );
    final horizontal = switch ((_moveLeft, _moveRight)) {
      (true, false) => -_moveSpeed,
      (false, true) => _moveSpeed,
      _ => 0.0,
    };
    final previousRect = _playerRect;
    var nextVelocity = Offset(horizontal, _velocity.dy + _gravity * dt);
    var nextX = (_position.dx + nextVelocity.dx * dt).clamp(
      0.0,
      _worldSize.width - _playerSize.width,
    );
    var nextY = _position.dy + nextVelocity.dy * dt;
    var grounded = false;

    if (nextVelocity.dy >= 0) {
      final nextRect = Rect.fromLTWH(
        nextX,
        nextY,
        _playerSize.width,
        _playerSize.height,
      );
      for (final platform in _platforms) {
        final crossesTop =
            previousRect.bottom <= platform.top + 3 &&
            nextRect.bottom >= platform.top;
        final horizontalOverlap =
            nextRect.right > platform.left + 3 &&
            nextRect.left < platform.right - 3;
        if (!crossesTop || !horizontalOverlap) continue;
        nextY = platform.top - _playerSize.height;
        nextVelocity = Offset(nextVelocity.dx, 0);
        grounded = true;
        break;
      }
    }

    final nextRect = Rect.fromLTWH(
      nextX,
      nextY,
      _playerSize.width,
      _playerSize.height,
    );
    if (nextY > _worldSize.height ||
        _hazards.any((hazard) => hazard.overlaps(nextRect))) {
      _respawn();
      return;
    }
    if (_exit.overlaps(nextRect)) {
      _complete();
      return;
    }
    setState(() {
      _position = Offset(nextX, nextY);
      _velocity = nextVelocity;
      _grounded = grounded;
    });
  }

  void _jump() {
    if (!_grounded || _cleared) return;
    setState(() {
      _velocity = Offset(_velocity.dx, _jumpSpeed);
      _grounded = false;
    });
  }

  void _respawn() {
    if (!mounted) return;
    setState(() {
      _position = _start;
      _velocity = Offset.zero;
      _grounded = false;
      _respawns++;
    });
  }

  void _restart() {
    setState(() {
      _position = _start;
      _velocity = Offset.zero;
      _grounded = false;
      _cleared = false;
      _respawns = 0;
      _startedAt = DateTime.now();
      _lastTick = null;
    });
    if (!_ticker.isActive) _ticker.start();
  }

  Future<void> _complete() async {
    if (_cleared) return;
    _cleared = true;
    _ticker.stop();
    final elapsed = DateTime.now().difference(_startedAt);
    await showDialog<void>(
      context: context,
      barrierDismissible: false,
      builder: (context) => AlertDialog(
        title: const Text('케이지 탈출 성공!'),
        content: Text(
          '기록: ${elapsed.inSeconds}.${(elapsed.inMilliseconds % 1000) ~/ 100}초\n'
          '리스폰: $_respawns회',
        ),
        actions: [
          TextButton(
            onPressed: () {
              Navigator.of(context).pop();
              _restart();
            },
            child: const Text('다시 하기'),
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
    backgroundColor: const Color(0xFF9BC7F5),
    appBar: AppBar(
      title: const Text('케이지 탈출'),
      actions: [
        Center(child: Text('리스폰 $_respawns')),
        IconButton(onPressed: _restart, icon: const Icon(Icons.refresh)),
      ],
    ),
    body: SafeArea(
      top: false,
      child: Column(
        children: [
          Expanded(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(14, 14, 14, 8),
              child: PixelGamePanel(
                padding: EdgeInsets.zero,
                child: SizedBox.expand(
                  child: CustomPaint(
                    painter: _CagePainter(
                      worldSize: _worldSize,
                      player: _playerRect,
                      platforms: _platforms,
                      hazards: _hazards,
                      exit: _exit,
                    ),
                  ),
                ),
              ),
            ),
          ),
          Padding(
            padding: const EdgeInsets.fromLTRB(20, 8, 20, 22),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                Row(
                  children: [
                    PixelControlButton(
                      tooltip: '왼쪽으로 이동',
                      onPressed: () => setState(() => _moveLeft = true),
                      onReleased: () => setState(() => _moveLeft = false),
                      child: const Icon(Icons.keyboard_arrow_left),
                    ),
                    const SizedBox(width: 16),
                    PixelControlButton(
                      tooltip: '오른쪽으로 이동',
                      onPressed: () => setState(() => _moveRight = true),
                      onReleased: () => setState(() => _moveRight = false),
                      child: const Icon(Icons.keyboard_arrow_right),
                    ),
                  ],
                ),
                PixelControlButton(
                  tooltip: '점프',
                  size: 68,
                  onPressed: _jump,
                  child: const Text(
                    'JUMP',
                    style: TextStyle(fontWeight: FontWeight.w700),
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    ),
  );
}

class _CagePainter extends CustomPainter {
  const _CagePainter({
    required this.worldSize,
    required this.player,
    required this.platforms,
    required this.hazards,
    required this.exit,
  });

  final Size worldSize;
  final Rect player;
  final List<Rect> platforms;
  final List<Rect> hazards;
  final Rect exit;

  @override
  void paint(Canvas canvas, Size size) {
    final scale = math.min(
      size.width / worldSize.width,
      size.height / worldSize.height,
    );
    final offset = Offset(
      (size.width - worldSize.width * scale) / 2,
      (size.height - worldSize.height * scale) / 2,
    );
    canvas.save();
    canvas.translate(offset.dx, offset.dy);
    canvas.scale(scale);
    canvas.drawRect(
      Offset.zero & worldSize,
      Paint()..color = const Color(0xFFDFF4FF),
    );
    final cagePaint = Paint()
      ..color = const Color(0x553A2A1F)
      ..strokeWidth = 5
      ..isAntiAlias = false;
    for (var x = 12.0; x < worldSize.width; x += 36) {
      canvas.drawLine(Offset(x, 0), Offset(x, worldSize.height), cagePaint);
    }
    final platformPaint = Paint()
      ..color = const Color(0xFF806040)
      ..isAntiAlias = false;
    for (final platform in platforms) {
      canvas.drawRect(platform, platformPaint);
      canvas.drawRect(
        Rect.fromLTWH(platform.left, platform.top, platform.width, 3),
        Paint()..color = pixelGameInk,
      );
    }
    final spikePaint = Paint()
      ..color = const Color(0xFFE25D5D)
      ..isAntiAlias = false;
    for (final hazard in hazards) {
      final path = Path()
        ..moveTo(hazard.left, hazard.bottom)
        ..lineTo(hazard.center.dx, hazard.top)
        ..lineTo(hazard.right, hazard.bottom)
        ..close();
      canvas.drawPath(path, spikePaint);
    }
    canvas.drawRect(exit, Paint()..color = const Color(0xFF7DAA6D));
    canvas.drawRect(
      Rect.fromLTWH(
        exit.left + 7,
        exit.top + 8,
        exit.width - 14,
        exit.height - 8,
      ),
      Paint()..color = pixelGameInk,
    );
    final playerPaint = Paint()
      ..color = const Color(0xFFFFE2C2)
      ..isAntiAlias = false;
    canvas.drawRect(player, playerPaint);
    canvas.drawRect(
      Rect.fromLTWH(player.left + 4, player.top - 4, 7, 7),
      playerPaint,
    );
    canvas.drawRect(
      Rect.fromLTWH(player.right - 11, player.top - 4, 7, 7),
      playerPaint,
    );
    canvas.drawRect(
      Rect.fromLTWH(player.left + 7, player.top + 11, 4, 4),
      Paint()..color = pixelGameInk,
    );
    canvas.drawRect(
      Rect.fromLTWH(player.right - 11, player.top + 11, 4, 4),
      Paint()..color = pixelGameInk,
    );
    canvas.restore();
  }

  @override
  bool shouldRepaint(covariant _CagePainter oldDelegate) =>
      oldDelegate.player != player;
}
