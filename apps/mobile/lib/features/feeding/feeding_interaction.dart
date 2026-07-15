import 'dart:async';

import 'package:flutter/material.dart';

import '../../core/widgets/pixel_glyph.dart';

@immutable
class FeedingState {
  const FeedingState({
    this.isActive = false,
    this.cheesePosition,
    this.isFeeding = false,
  });

  final bool isActive;
  final Offset? cheesePosition;
  final bool isFeeding;
}

class FeedingController extends ChangeNotifier {
  FeedingController({this.animationDuration = const Duration(seconds: 1)});

  static const cheeseSize = 36.0;

  final Duration animationDuration;
  FeedingState _state = const FeedingState();
  bool _disposed = false;

  FeedingState get state => _state;

  void activate() {
    if (_state.isFeeding) return;
    _setState(const FeedingState(isActive: true));
  }

  void cancel() {
    if (_state.isFeeding) return;
    _setState(const FeedingState());
  }

  Future<bool> dragCheese(Offset position, Rect? mouseCollider) async {
    if (!_state.isActive || _state.isFeeding) return false;
    _setState(FeedingState(isActive: true, cheesePosition: position));
    if (mouseCollider == null) return false;

    final cheeseCollider = Rect.fromCenter(
      center: position,
      width: cheeseSize,
      height: cheeseSize,
    );
    final tightMouseCollider = mouseCollider.deflate(
      mouseCollider.shortestSide * 0.14,
    );
    if (!cheeseCollider.overlaps(tightMouseCollider)) return false;

    _setState(const FeedingState(isFeeding: true));
    await playEatingAnimation();
    return true;
  }

  /// Temporary animation boundary. Replace only this method when an eating
  /// sprite sheet or Lottie/Rive asset becomes available.
  Future<void> playEatingAnimation() async {
    await Future<void>.delayed(animationDuration);
    if (_disposed) return;
    _setState(const FeedingState());
  }

  void _setState(FeedingState next) {
    if (_disposed) return;
    _state = next;
    notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}

class FeedingGestureLayer extends StatelessWidget {
  const FeedingGestureLayer({
    super.key,
    required this.state,
    required this.mouseCollider,
    required this.onPointer,
  });

  final FeedingState state;
  final Rect? Function() mouseCollider;
  final ValueChanged<Offset> onPointer;

  @override
  Widget build(BuildContext context) {
    final collider = mouseCollider();
    final cheesePosition = state.cheesePosition;
    return Stack(
      children: [
        Positioned.fill(
          child: IgnorePointer(
            ignoring: !state.isActive,
            child: GestureDetector(
              behavior: HitTestBehavior.opaque,
              onTapDown: (details) => onPointer(details.localPosition),
              onPanStart: (details) => onPointer(details.localPosition),
              onPanUpdate: (details) => onPointer(details.localPosition),
            ),
          ),
        ),
        if (state.isActive && state.cheesePosition == null)
          const Positioned(
            left: 24,
            right: 24,
            bottom: 90,
            child: IgnorePointer(
              child: Text(
                '화면을 누르고 치즈를 생쥐에게 드래그하세요!',
                textAlign: TextAlign.center,
                style: TextStyle(
                  color: Color(0xFFFFF4D6),
                  fontWeight: FontWeight.w700,
                  shadows: [
                    Shadow(color: Color(0xFF3A2A1F), offset: Offset(2, 2)),
                  ],
                ),
              ),
            ),
          ),
        if (state.isActive && cheesePosition != null)
          AnimatedPositioned(
            duration: const Duration(milliseconds: 55),
            curve: Curves.easeOut,
            left: cheesePosition.dx - FeedingController.cheeseSize / 2,
            top: cheesePosition.dy - FeedingController.cheeseSize / 2,
            child: const IgnorePointer(
              child: PixelGlyphIcon(
                PixelGlyph.cheese,
                size: FeedingController.cheeseSize,
                accentColor: Color(0xFFFFC857),
              ),
            ),
          ),
        if (state.isFeeding && collider != null)
          Positioned(
            left: collider.center.dx - 34,
            top: collider.top - 22,
            child: const IgnorePointer(child: _YumEffect()),
          ),
      ],
    );
  }
}

class _YumEffect extends StatelessWidget {
  const _YumEffect();

  @override
  Widget build(BuildContext context) => TweenAnimationBuilder<double>(
    tween: Tween(begin: 0.7, end: 1.15),
    duration: const Duration(milliseconds: 420),
    curve: Curves.elasticOut,
    builder: (context, scale, child) =>
        Transform.scale(scale: scale, child: child),
    child: const DecoratedBox(
      decoration: BoxDecoration(
        color: Color(0xFFFFC857),
        border: Border.fromBorderSide(
          BorderSide(color: Color(0xFF3A2A1F), width: 2),
        ),
      ),
      child: Padding(
        padding: EdgeInsets.symmetric(horizontal: 9, vertical: 4),
        child: Text('YUM!', style: TextStyle(fontWeight: FontWeight.w700)),
      ),
    ),
  );
}
