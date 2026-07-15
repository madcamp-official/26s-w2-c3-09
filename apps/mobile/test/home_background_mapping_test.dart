import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper_character_assets/character_assets.dart';

void main() {
  test('managed folder backgrounds are shifted by one slot', () {
    const expected = <String>[
      'backgrounds/background_5.png',
      'backgrounds/background_1.png',
      'backgrounds/background_2.png',
      'backgrounds/background_3.png',
      'backgrounds/background_4.png',
    ];

    for (var index = 0; index < expected.length; index++) {
      expect(mousekeeperHomeBackgroundAssetForIndex(index), expected[index]);
    }
  });
}
