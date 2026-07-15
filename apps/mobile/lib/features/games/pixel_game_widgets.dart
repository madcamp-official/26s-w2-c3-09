import 'package:flutter/material.dart';

const pixelGameInk = Color(0xFF3A2A1F);
const pixelGamePaper = Color(0xFFFFF4D6);
const pixelGameAccent = Color(0xFFFFC857);
const pixelGameSky = Color(0xFFAED9E0);

class PixelGamePanel extends StatelessWidget {
  const PixelGamePanel({super.key, required this.child, this.padding});

  final Widget child;
  final EdgeInsetsGeometry? padding;

  @override
  Widget build(BuildContext context) => Container(
    margin: const EdgeInsets.only(right: 5, bottom: 5),
    padding: padding ?? const EdgeInsets.all(14),
    decoration: BoxDecoration(
      color: pixelGamePaper,
      border: Border.all(color: pixelGameInk, width: 2),
      boxShadow: const [BoxShadow(color: pixelGameInk, offset: Offset(5, 5))],
    ),
    child: child,
  );
}

class PixelControlButton extends StatelessWidget {
  const PixelControlButton({
    super.key,
    required this.child,
    required this.onPressed,
    this.onReleased,
    this.size = 54,
    this.tooltip,
  });

  final Widget child;
  final VoidCallback onPressed;
  final VoidCallback? onReleased;
  final double size;
  final String? tooltip;

  @override
  Widget build(BuildContext context) {
    final button = Listener(
      onPointerDown: (_) => onPressed(),
      onPointerUp: (_) => onReleased?.call(),
      onPointerCancel: (_) => onReleased?.call(),
      child: Container(
        width: size,
        height: size,
        alignment: Alignment.center,
        decoration: BoxDecoration(
          color: pixelGamePaper,
          border: Border.all(color: pixelGameInk, width: 2),
          boxShadow: const [
            BoxShadow(color: pixelGameInk, offset: Offset(4, 4)),
          ],
        ),
        child: child,
      ),
    );
    return tooltip == null ? button : Tooltip(message: tooltip!, child: button);
  }
}
