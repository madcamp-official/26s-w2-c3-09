import { rankDirectoryCandidates } from './path-candidates';

describe('rankDirectoryCandidates', () => {
  it('ranks verified images for img without inventing paths', () => {
    const result = rankDirectoryCandidates('img', [
      'docs',
      'images',
      'image-backups',
    ]);
    expect(result.map((item) => item.relativePath)).toEqual(['images']);
    expect(result[0]!.score).toBeGreaterThanOrEqual(0.4);
  });

  it('normalizes case and returns at most three verified candidates', () => {
    const result = rankDirectoryCandidates('REPORTS', [
      'Reports',
      'reports-old',
      'reports-new',
      'reports-2025',
    ]);
    expect(result[0]).toEqual({ relativePath: 'Reports', score: 1 });
    expect(result).toHaveLength(3);
    expect(result.every((item) => !item.relativePath.includes('..'))).toBe(
      true,
    );
  });
});
