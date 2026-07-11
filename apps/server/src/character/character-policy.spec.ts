import {
  characterSelectionIsUnlocked,
  FIRST_CHARACTER_UNLOCK_AFFINITY,
  mergeCharacterAppearance,
  unlockedCharacterItems,
} from './character-policy';

describe('character unlock policy', () => {
  it('keeps the second appearance and theme locked below affinity 3', () => {
    const unlocked = unlockedCharacterItems(
      FIRST_CHARACTER_UNLOCK_AFFINITY - 1,
    );
    expect(unlocked).toEqual(['fur:brown', 'accessory:none', 'theme:warm']);
    expect(
      characterSelectionIsUnlocked(unlocked, {
        furVariant: 'cream',
        roomTheme: 'forest',
      }),
    ).toBe(false);
  });

  it('unlocks cosmetic choices without changing file permissions', () => {
    const unlocked = unlockedCharacterItems(FIRST_CHARACTER_UNLOCK_AFFINITY);
    expect(
      characterSelectionIsUnlocked(unlocked, {
        furVariant: 'cream',
        accessory: 'scarf',
        roomTheme: 'forest',
      }),
    ).toBe(true);
    expect(unlocked.every((item) => /^(fur|accessory|theme):/.test(item))).toBe(
      true,
    );
  });

  it('merges a partial animation update without losing appearance choices', () => {
    expect(
      mergeCharacterAppearance(
        { furVariant: 'cream', accessory: 'scarf', animationsEnabled: true },
        { animationsEnabled: false },
      ),
    ).toEqual({
      furVariant: 'cream',
      accessory: 'scarf',
      animationsEnabled: false,
    });
  });
});
