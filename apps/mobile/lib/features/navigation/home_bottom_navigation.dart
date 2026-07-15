import 'package:flutter/material.dart';

enum HomeMenuAction { files, games, feeding, wardrobe }

const _menuIconAssets = <HomeMenuAction, String>{
  HomeMenuAction.files: 'assets/images/home_navigation/folder.png',
  HomeMenuAction.games: 'assets/images/home_navigation/console.png',
  HomeMenuAction.feeding: 'assets/images/home_navigation/cheese.png',
  HomeMenuAction.wardrobe: 'assets/images/home_navigation/hanger.png',
};

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
            _item(HomeMenuAction.files, '파일 목록'),
            _item(HomeMenuAction.games, '미니게임'),
            _item(HomeMenuAction.feeding, '치즈 주기'),
            _item(HomeMenuAction.wardrobe, '생쥐 의상실'),
          ],
        ),
      ),
    ),
  );

  Widget _item(HomeMenuAction action, String tooltip) => Expanded(
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
            child: Image.asset(
              _menuIconAssets[action]!,
              key: ValueKey('home-menu-icon-${action.name}'),
              width: 46,
              height: 46,
              fit: BoxFit.contain,
              filterQuality: FilterQuality.none,
            ),
          ),
        ),
      ),
    ),
  );
}
