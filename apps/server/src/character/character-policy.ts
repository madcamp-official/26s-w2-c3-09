const baseItems = ['fur:brown', 'accessory:none', 'theme:warm'] as const;
const firstUnlock = ['fur:cream', 'accessory:scarf', 'theme:forest'] as const;

export const FIRST_CHARACTER_UNLOCK_AFFINITY = 3;

export interface CharacterAppearance {
  furVariant: 'brown' | 'cream';
  accessory: 'none' | 'scarf';
  animationsEnabled: boolean;
}

export function mergeCharacterAppearance(
  current: unknown,
  patch: Partial<CharacterAppearance>,
): CharacterAppearance {
  const value =
    current && typeof current === 'object'
      ? (current as Record<string, unknown>)
      : {};
  return {
    furVariant:
      value.furVariant === 'cream' || value.furVariant === 'brown'
        ? value.furVariant
        : 'brown',
    accessory:
      value.accessory === 'scarf' || value.accessory === 'none'
        ? value.accessory
        : 'none',
    animationsEnabled:
      typeof value.animationsEnabled === 'boolean'
        ? value.animationsEnabled
        : true,
    ...patch,
  };
}

export function unlockedCharacterItems(affinityTotal: number): string[] {
  return affinityTotal >= FIRST_CHARACTER_UNLOCK_AFFINITY
    ? [...baseItems, ...firstUnlock]
    : [...baseItems];
}

export function characterSelectionIsUnlocked(
  unlocked: readonly string[],
  selection: {
    furVariant?: string;
    accessory?: string;
    roomTheme?: string | null;
  },
) {
  return (
    (!selection.furVariant ||
      unlocked.includes(`fur:${selection.furVariant}`)) &&
    (!selection.accessory ||
      unlocked.includes(`accessory:${selection.accessory}`)) &&
    (!selection.roomTheme || unlocked.includes(`theme:${selection.roomTheme}`))
  );
}
