import 'package:flutter/material.dart';

import '../../core/widgets/pixel_glyph.dart';

enum HomeMenuAction { files, games, feeding, wardrobe }

class HomeBottomNavigation extends StatelessWidget {
  const HomeBottomNavigation({
    super.key,
    required this.selected,
    required this.onSelected,
  });

  final HomeMenuAction? selected;
  final ValueChanged<HomeMenuAction> onSelected;

  @override
  Widget build(BuildContext context) => Semantics(
    container: true,
    label: '하단 메뉴',
    child: DecoratedBox(
      decoration: BoxDecoration(
        color: const Color(0xFFFFF4D6).withValues(alpha: 0.96),
        border: Border.all(color: const Color(0xFF3A2A1F), width: 2),
        boxShadow: const [
          BoxShadow(color: Color(0xFF3A2A1F), offset: Offset(5, 5)),
        ],
      ),
      child: SizedBox(
        height: 66,
        child: Row(
          children: [
            _item(HomeMenuAction.files, PixelGlyph.folder, '파일 목록'),
            _item(HomeMenuAction.games, PixelGlyph.puzzle, '미니게임'),
            _item(HomeMenuAction.feeding, PixelGlyph.cheese, '치즈 주기'),
            _item(HomeMenuAction.wardrobe, PixelGlyph.hanger, '생쥐 의상실'),
          ],
        ),
      ),
    ),
  );

  Widget _item(HomeMenuAction action, PixelGlyph glyph, String tooltip) =>
      Expanded(
        child: Semantics(
          button: true,
          selected: selected == action,
          label: tooltip,
          child: Tooltip(
            message: tooltip,
            child: InkWell(
              key: ValueKey('home-menu-${action.name}'),
              onTap: () => onSelected(action),
              child: AnimatedContainer(
                duration: const Duration(milliseconds: 120),
                margin: const EdgeInsets.all(6),
                decoration: BoxDecoration(
                  color: selected == action
                      ? const Color(0xFFFFD778)
                      : Colors.transparent,
                  border: selected == action
                      ? Border.all(color: const Color(0xFF3A2A1F), width: 2)
                      : null,
                ),
                alignment: Alignment.center,
                child: PixelGlyphIcon(
                  glyph,
                  size: 38,
                  accentColor: switch (action) {
                    HomeMenuAction.files => const Color(0xFF9BC7F5),
                    HomeMenuAction.games => const Color(0xFFE78596),
                    HomeMenuAction.feeding => const Color(0xFFFFC857),
                    HomeMenuAction.wardrobe => const Color(0xFFB9A7E8),
                  },
                ),
              ),
            ),
          ),
        ),
      );
}
