import 'package:flutter/material.dart';
import 'package:mousekeeper_character_assets/character_assets.dart';

import 'mousekeeper_motion.dart';

/// Read-only MVP character information. Persisted legacy appearance and room
/// theme fields are deliberately ignored so affinity cannot mutate visuals.
class CharacterSettingsPage extends StatelessWidget {
  const CharacterSettingsPage({super.key, required this.initialCharacter});

  final Map<String, dynamic> initialCharacter;

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: AppBar(title: const Text('MOUSEKEEPER 캐릭터')),
    body: ListView(
      padding: const EdgeInsets.all(24),
      children: [
        Center(
          child: SizedBox.square(
            dimension: 144,
            child: MouseKeeperMotionImage(motion: MouseKeeperMotion.stand),
          ),
        ),
        const SizedBox(height: 24),
        const Card(
          child: ListTile(
            leading: Icon(Icons.lock_outline),
            title: Text('기본 외형과 기본 방 테마 사용 중'),
            subtitle: Text('MVP에서는 호감도와 관계없이 같은 외형과 테마를 사용합니다.'),
          ),
        ),
        Card(
          child: ListTile(
            leading: const Icon(Icons.favorite_outline),
            title: Text('호감도 ${initialCharacter['affinityTotal'] ?? 0}'),
            subtitle: const Text('호감도 기록과 완료 대사는 유지됩니다.'),
          ),
        ),
        if (initialCharacter['riveAssetStatus'] == 'UNCONFIGURED')
          const Card(
            color: Color(0xFFFFF3E0),
            child: ListTile(
              leading: Icon(Icons.animation_outlined),
              title: Text('Rive 애니메이션 미설정'),
              subtitle: Text('검증된 기본 PNG 상태 모션을 사용합니다. 오류 코드: UNCONFIGURED'),
            ),
          ),
      ],
    ),
  );
}
