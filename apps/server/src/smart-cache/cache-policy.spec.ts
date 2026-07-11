import { matchesExcludedPattern } from './cache-policy';

describe('smart cache excluded patterns', () => {
  it('supports bounded path globs without treating regex text as executable', () => {
    expect(
      matchesExcludedPattern('private/report.pdf', [
        'private/**',
        '*.tmp',
        'literal[1].txt',
      ]),
    ).toBe(true);
    expect(matchesExcludedPattern('notes.tmp', ['*.tmp'])).toBe(true);
    expect(matchesExcludedPattern('nested/notes.tmp', ['*.tmp'])).toBe(false);
    expect(matchesExcludedPattern('literal1.txt', ['literal[1].txt'])).toBe(
      false,
    );
  });
});
