import 'package:flutter/material.dart';

enum PixelGlyph { folder, puzzle, cheese, hanger, mouse, exitDoor, bug }

class PixelGlyphIcon extends StatelessWidget {
  const PixelGlyphIcon(
    this.glyph, {
    super.key,
    this.size = 32,
    this.color = const Color(0xFF3A2A1F),
    this.accentColor = const Color(0xFFFFC857),
  });

  final PixelGlyph glyph;
  final double size;
  final Color color;
  final Color accentColor;

  @override
  Widget build(BuildContext context) => SizedBox.square(
    dimension: size,
    child: CustomPaint(
      painter: _PixelGlyphPainter(
        pattern: _patterns[glyph]!,
        color: color,
        accentColor: accentColor,
      ),
    ),
  );
}

class _PixelGlyphPainter extends CustomPainter {
  const _PixelGlyphPainter({
    required this.pattern,
    required this.color,
    required this.accentColor,
  });

  final List<String> pattern;
  final Color color;
  final Color accentColor;

  @override
  void paint(Canvas canvas, Size size) {
    final rows = pattern.length;
    final columns = pattern.fold<int>(
      0,
      (longest, row) => row.length > longest ? row.length : longest,
    );
    final pixel = size.shortestSide / (rows > columns ? rows : columns);
    final origin = Offset(
      (size.width - columns * pixel) / 2,
      (size.height - rows * pixel) / 2,
    );
    final outline = Paint()
      ..isAntiAlias = false
      ..color = color;
    final accent = Paint()
      ..isAntiAlias = false
      ..color = accentColor;
    final highlight = Paint()
      ..isAntiAlias = false
      ..color = const Color(0xFFFFF4D6);

    for (var y = 0; y < rows; y++) {
      final row = pattern[y];
      for (var x = 0; x < row.length; x++) {
        final paint = switch (row[x]) {
          '1' => outline,
          '2' => accent,
          '3' => highlight,
          _ => null,
        };
        if (paint == null) continue;
        canvas.drawRect(
          Rect.fromLTWH(
            origin.dx + x * pixel,
            origin.dy + y * pixel,
            pixel.ceilToDouble(),
            pixel.ceilToDouble(),
          ),
          paint,
        );
      }
    }
  }

  @override
  bool shouldRepaint(covariant _PixelGlyphPainter oldDelegate) =>
      oldDelegate.pattern != pattern ||
      oldDelegate.color != color ||
      oldDelegate.accentColor != accentColor;
}

const _patterns = <PixelGlyph, List<String>>{
  PixelGlyph.folder: [
    '..........',
    '.1111.....',
    '.12211111.',
    '112222221.',
    '122222221.',
    '122222221.',
    '122222221.',
    '111111111.',
  ],
  PixelGlyph.puzzle: [
    '...111....',
    '..12221...',
    '111222111.',
    '122222221.',
    '112222211.',
    '..12221...',
    '..11111...',
    '..........',
  ],
  PixelGlyph.cheese: [
    '........1.',
    '......112.',
    '....11222.',
    '..1122222.',
    '.12212222.',
    '.12222122.',
    '.12122222.',
    '.11111111.',
  ],
  PixelGlyph.hanger: [
    '....11....',
    '...1221...',
    '...1111...',
    '....11....',
    '...1111...',
    '..11..11..',
    '.11....11.',
    '1111111111',
  ],
  PixelGlyph.mouse: [
    '.11....11.',
    '1221..1221',
    '1221111221',
    '.12222221.',
    '.21322312.',
    '.22211222.',
    '..122221..',
    '...1111...',
  ],
  PixelGlyph.exitDoor: [
    '..111111..',
    '..122221..',
    '..122221..',
    '..122321..',
    '..122221..',
    '..122221..',
    '..122221..',
    '1111111111',
  ],
  PixelGlyph.bug: [
    '...11.11..',
    '....11....',
    '..112211..',
    '.12222221.',
    '1122222211',
    '.12222221.',
    '..112211..',
    '...1..1...',
  ],
};
