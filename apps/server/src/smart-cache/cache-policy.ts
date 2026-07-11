function globRegex(pattern: string) {
  const normalized = pattern.trim().replaceAll('\\', '/');
  let source = '^';
  for (let index = 0; index < normalized.length; index++) {
    const character = normalized[index]!;
    if (character === '*' && normalized[index + 1] === '*') {
      source += '.*';
      index++;
    } else if (character === '*') {
      source += '[^/]*';
    } else if (character === '?') {
      source += '[^/]';
    } else {
      source += character.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    }
  }
  return new RegExp(`${source}$`, 'u');
}

export function matchesExcludedPattern(
  relativePath: string,
  patterns: readonly string[],
) {
  const normalized = relativePath.replaceAll('\\', '/');
  return patterns.some((pattern) => globRegex(pattern).test(normalized));
}
