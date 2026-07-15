import 'dart:async';
import 'dart:math';

import 'package:flutter/material.dart';

import '../../core/widgets/pixel_glyph.dart';
import 'pixel_game_widgets.dart';

enum WhackTargetKind { cheese, bug }

class WhackGamePage extends StatefulWidget {
  const WhackGamePage({super.key});

  @override
  State<WhackGamePage> createState() => _WhackGamePageState();
}

class _WhackGamePageState extends State<WhackGamePage> {
  final _random = Random();
  Timer? _countdownTimer;
  Timer? _spawnTimer;
  int _remainingSeconds = 30;
  int _score = 0;
  int _combo = 0;
  int _bestCombo = 0;
  int? _activeIndex;
  WhackTargetKind? _activeKind;
  bool _finished = false;

  @override
  void initState() {
    super.initState();
    _startGame();
  }

  @override
  void dispose() {
    _countdownTimer?.cancel();
    _spawnTimer?.cancel();
    super.dispose();
  }

  void _startGame() {
    _countdownTimer?.cancel();
    _spawnTimer?.cancel();
    _remainingSeconds = 30;
    _score = 0;
    _combo = 0;
    _bestCombo = 0;
    _finished = false;
    _spawnTarget();
    _spawnTimer = Timer.periodic(
      const Duration(milliseconds: 620),
      (_) => _spawnTarget(),
    );
    _countdownTimer = Timer.periodic(const Duration(seconds: 1), (_) {
      if (!mounted || _finished) return;
      setState(() => _remainingSeconds--);
      if (_remainingSeconds <= 0) _finishGame();
    });
    if (mounted) setState(() {});
  }

  void _spawnTarget() {
    if (!mounted || _finished) return;
    setState(() {
      _activeIndex = _random.nextInt(9);
      _activeKind = _random.nextDouble() < 0.76
          ? WhackTargetKind.cheese
          : WhackTargetKind.bug;
    });
  }

  void _tapCell(int index) {
    if (_finished) return;
    setState(() {
      if (_activeIndex != index) {
        _score = max(0, _score - 3);
        _combo = 0;
      } else if (_activeKind == WhackTargetKind.cheese) {
        _combo++;
        _bestCombo = max(_bestCombo, _combo);
        _score += 10 + (_combo - 1) * 2;
        _activeIndex = null;
        _activeKind = null;
      } else {
        _score = max(0, _score - 10);
        _combo = 0;
        _activeIndex = null;
        _activeKind = null;
      }
    });
  }

  Future<void> _finishGame() async {
    if (_finished) return;
    _finished = true;
    _countdownTimer?.cancel();
    _spawnTimer?.cancel();
    setState(() {
      _remainingSeconds = 0;
      _activeIndex = null;
      _activeKind = null;
    });
    await showDialog<void>(
      context: context,
      barrierDismissible: false,
      builder: (context) => AlertDialog(
        title: const Text('30초 완료!'),
        content: Text('최종 점수: $_score점\n최고 콤보: $_bestCombo회'),
        actions: [
          TextButton(
            onPressed: () {
              Navigator.of(context).pop();
              _startGame();
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
    backgroundColor: const Color(0xFFE6D0A8),
    appBar: AppBar(
      title: const Text('치즈 잡기'),
      actions: [
        Center(child: Text('$_remainingSeconds초')),
        const SizedBox(width: 16),
      ],
    ),
    body: SafeArea(
      top: false,
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          children: [
            Row(
              children: [
                Expanded(
                  child: PixelGamePanel(
                    child: Text(
                      '점수 $_score',
                      textAlign: TextAlign.center,
                      style: const TextStyle(fontWeight: FontWeight.w700),
                    ),
                  ),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: PixelGamePanel(
                    child: Text(
                      '콤보 x$_combo',
                      textAlign: TextAlign.center,
                      style: const TextStyle(fontWeight: FontWeight.w700),
                    ),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 16),
            Expanded(
              child: Center(
                child: AspectRatio(
                  aspectRatio: 1,
                  child: GridView.builder(
                    physics: const NeverScrollableScrollPhysics(),
                    gridDelegate:
                        const SliverGridDelegateWithFixedCrossAxisCount(
                          crossAxisCount: 3,
                          crossAxisSpacing: 12,
                          mainAxisSpacing: 12,
                        ),
                    itemCount: 9,
                    itemBuilder: (context, index) => _WhackHole(
                      index: index,
                      kind: _activeIndex == index ? _activeKind : null,
                      onTap: () => _tapCell(index),
                    ),
                  ),
                ),
              ),
            ),
            const SizedBox(height: 14),
            const Text('치즈는 +점수 · 벌레는 -10점 · 빈칸은 -3점'),
          ],
        ),
      ),
    ),
  );
}

class _WhackHole extends StatelessWidget {
  const _WhackHole({
    required this.index,
    required this.kind,
    required this.onTap,
  });

  final int index;
  final WhackTargetKind? kind;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) => Semantics(
    button: true,
    label: '${index + 1}번 구멍',
    child: InkWell(
      key: ValueKey('whack-hole-$index'),
      onTap: onTap,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: const Color(0xFF806040),
          border: Border.all(color: pixelGameInk, width: 3),
          boxShadow: const [
            BoxShadow(color: pixelGameInk, offset: Offset(4, 4)),
          ],
        ),
        child: Center(
          child: AnimatedSwitcher(
            duration: const Duration(milliseconds: 100),
            child: switch (kind) {
              WhackTargetKind.cheese => const PixelGlyphIcon(
                PixelGlyph.cheese,
                key: ValueKey('cheese'),
                size: 58,
              ),
              WhackTargetKind.bug => const PixelGlyphIcon(
                PixelGlyph.bug,
                key: ValueKey('bug'),
                size: 58,
                accentColor: Color(0xFF7DAA6D),
              ),
              null => const SizedBox(key: ValueKey('empty')),
            },
          ),
        ),
      ),
    ),
  );
}
