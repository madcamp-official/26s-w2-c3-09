import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/core/models/character_state.dart';

void main() {
  test('all nine contract values round-trip through the typed parser', () {
    const expected = <String, CharacterState>{
      'IDLE': CharacterState.idle,
      'CONNECTING': CharacterState.connecting,
      'ANALYZING': CharacterState.analyzing,
      'WAITING_APPROVAL': CharacterState.waitingApproval,
      'WORKING': CharacterState.working,
      'SUCCESS': CharacterState.success,
      'ERROR': CharacterState.error,
      'USER_WORKING': CharacterState.userWorking,
      'OFFLINE': CharacterState.offline,
    };

    expect(CharacterState.values, hasLength(9));
    for (final entry in expected.entries) {
      expect(parseCharacterState(entry.key), entry.value);
      expect(entry.value.wireValue, entry.key);
    }
  });

  test('unknown or non-string character states fail closed', () {
    expect(parseCharacterState('success'), isNull);
    expect(parseCharacterState('MADE_UP'), isNull);
    expect(parseCharacterState(1), isNull);
    expect(parseCharacterState(null), isNull);
  });
}
