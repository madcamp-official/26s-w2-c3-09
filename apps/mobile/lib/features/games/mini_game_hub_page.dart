import 'package:flutter/material.dart';

import '../../core/widgets/pixel_glyph.dart';
import 'mini_game_router.dart';
import 'pixel_game_widgets.dart';

class MiniGameHubPage extends StatelessWidget {
  const MiniGameHubPage({super.key});

  @override
  Widget build(BuildContext context) => Scaffold(
    backgroundColor: const Color(0xFFAED9E0),
    appBar: AppBar(title: const Text('미니게임 허브')),
    body: ListView(
      padding: const EdgeInsets.fromLTRB(18, 22, 18, 28),
      children: [
        const Text(
          'PLAY ROOM',
          textAlign: TextAlign.center,
          style: TextStyle(fontSize: 28, fontWeight: FontWeight.w700),
        ),
        const SizedBox(height: 8),
        const Text('플레이할 게임을 골라주세요.', textAlign: TextAlign.center),
        const SizedBox(height: 24),
        _GameCard(
          key: const ValueKey('game-card-maze'),
          title: '미로 찾기',
          description: '10×10 미로를 빠져나가 치즈를 찾으세요.',
          glyph: PixelGlyph.cheese,
          color: const Color(0xFFFFC857),
          onTap: () =>
              Navigator.of(context).push(miniGameRoute(MiniGameKind.maze)),
        ),
        const SizedBox(height: 15),
        _GameCard(
          key: const ValueKey('game-card-whack'),
          title: '치즈 잡기',
          description: '30초 동안 치즈를 잡고 벌레는 피하세요.',
          glyph: PixelGlyph.bug,
          color: const Color(0xFF7DAA6D),
          onTap: () =>
              Navigator.of(context).push(miniGameRoute(MiniGameKind.whack)),
        ),
        const SizedBox(height: 15),
        _GameCard(
          key: const ValueKey('game-card-cage'),
          title: '케이지 탈출',
          description: '점프와 이동으로 가시를 피해 출구에 도달하세요.',
          glyph: PixelGlyph.exitDoor,
          color: const Color(0xFFB9A7E8),
          onTap: () => Navigator.of(
            context,
          ).push(miniGameRoute(MiniGameKind.cageEscape)),
        ),
      ],
    ),
  );
}

class _GameCard extends StatelessWidget {
  const _GameCard({
    super.key,
    required this.title,
    required this.description,
    required this.glyph,
    required this.color,
    required this.onTap,
  });

  final String title;
  final String description;
  final PixelGlyph glyph;
  final Color color;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) => InkWell(
    onTap: onTap,
    child: PixelGamePanel(
      child: Row(
        children: [
          DecoratedBox(
            decoration: BoxDecoration(
              color: color.withValues(alpha: 0.35),
              border: Border.all(color: pixelGameInk, width: 2),
            ),
            child: Padding(
              padding: const EdgeInsets.all(10),
              child: PixelGlyphIcon(glyph, size: 56, accentColor: color),
            ),
          ),
          const SizedBox(width: 16),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  title,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(
                    fontSize: 19,
                    fontWeight: FontWeight.w700,
                  ),
                ),
                const SizedBox(height: 7),
                Text(description, maxLines: 3, overflow: TextOverflow.ellipsis),
              ],
            ),
          ),
          const Icon(Icons.chevron_right),
        ],
      ),
    ),
  );
}
