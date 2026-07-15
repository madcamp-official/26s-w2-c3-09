import 'package:flutter/material.dart';

import '../../core/widgets/pixel_glyph.dart';

class MouseWardrobePage extends StatelessWidget {
  const MouseWardrobePage({super.key});

  @override
  Widget build(BuildContext context) => Scaffold(
    backgroundColor: const Color(0xFFFFE9D8),
    appBar: AppBar(
      title: const Text(
        '생쥐의 의상실',
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
      ),
    ),
    body: Center(
      child: Padding(
        padding: const EdgeInsets.all(28),
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: const Color(0xFFFFF4D6),
            border: Border.all(color: const Color(0xFF3A2A1F), width: 3),
            boxShadow: const [
              BoxShadow(color: Color(0xFF3A2A1F), offset: Offset(7, 7)),
            ],
          ),
          child: const Padding(
            padding: EdgeInsets.symmetric(horizontal: 28, vertical: 38),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                PixelGlyphIcon(
                  PixelGlyph.hanger,
                  size: 92,
                  accentColor: Color(0xFFB9A7E8),
                ),
                SizedBox(height: 24),
                Text(
                  '생쥐의 의상실에\n오신 것을 환영합니다!',
                  textAlign: TextAlign.center,
                  style: TextStyle(fontSize: 22, fontWeight: FontWeight.w700),
                ),
                SizedBox(height: 14),
                Text('새로운 옷과 소품은 곧 준비됩니다.', textAlign: TextAlign.center),
              ],
            ),
          ),
        ),
      ),
    ),
  );
}
