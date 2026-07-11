import { rateLimitKey, requestLimit } from './rate-limit';

describe('rate limit policy', () => {
  it('applies stricter limits to pairing and transfer endpoints', () => {
    expect(requestLimit('/v1/pairing-sessions')).toBe(10);
    expect(requestLimit('/v1/file-transfers/id/download')).toBe(30);
    expect(requestLimit('/v1/rooms')).toBe(120);
  });

  it('does not store a raw IP address in Redis keys', () => {
    const key = rateLimitKey(
      'a-secret-with-at-least-32-characters',
      '203.0.113.10',
      '/v1/rooms',
      123,
    );
    expect(key).not.toContain('203.0.113.10');
    expect(key).toMatch(/^rate-limit:api:123:[a-f0-9]{64}$/);
  });
});
