export type PathCandidate = { relativePath: string; score: number };

/** Ranks only caller-supplied, already verified directory paths. */
export function rankDirectoryCandidates(
  requested: string,
  verifiedDirectories: string[],
  limit = 3,
): PathCandidate[] {
  const needle = normalizeName(requested);
  if (!needle) return [];
  return [...new Set(verifiedDirectories)]
    .map((relativePath) => ({
      relativePath,
      score: similarity(needle, normalizeName(lastSegment(relativePath))),
    }))
    .filter((candidate) => candidate.score >= 0.4)
    .sort(
      (left, right) =>
        right.score - left.score ||
        left.relativePath.localeCompare(right.relativePath),
    )
    .slice(0, Math.max(0, Math.min(limit, 3)));
}

function normalizeName(value: string) {
  return value
    .normalize('NFKC')
    .toLocaleLowerCase('en-US')
    .replace(/[\s_.-]+/g, ' ')
    .trim();
}

function lastSegment(value: string) {
  return value.replace(/\\/g, '/').split('/').filter(Boolean).at(-1) ?? '';
}

function similarity(left: string, right: string) {
  if (left === right) return 1;
  if (left.startsWith(right) || right.startsWith(left)) {
    return (
      0.85 *
      (Math.min(left.length, right.length) /
        Math.max(left.length, right.length))
    );
  }
  const distance = levenshtein(left, right);
  return 1 - distance / Math.max(left.length, right.length, 1);
}

function levenshtein(left: string, right: string) {
  const previous = Array.from(
    { length: right.length + 1 },
    (_, index) => index,
  );
  for (let i = 1; i <= left.length; i += 1) {
    let diagonal = previous[0];
    previous[0] = i;
    for (let j = 1; j <= right.length; j += 1) {
      const above = previous[j];
      previous[j] = Math.min(
        previous[j] + 1,
        previous[j - 1] + 1,
        diagonal + (left[i - 1] === right[j - 1] ? 0 : 1),
      );
      diagonal = above;
    }
  }
  return previous[right.length];
}
