import { rateLimitKey, requestLimit } from './rate-limit';

describe('rate limit policy', () => {
  it('applies stricter limits to pairing and transfer endpoints', () => {
    expect(requestLimit('/v1/pairing-sessions')).toBe(10);
    expect(
      requestLimit(
        '/v1/pairing-sessions/018f4c7b-1ad6-7c95-bf34-5e45881f98a1/status',
        'GET',
      ),
    ).toBe(60);
    expect(
      requestLimit(
        '/v1/pairing-sessions/018f4c7b-1ad6-7c95-bf34-5e45881f98a1/status',
        'POST',
      ),
    ).toBe(10);
    expect(requestLimit('/v1/file-transfers/id/download')).toBe(30);
    expect(requestLimit('/v1/rooms')).toBe(120);
  });

  it('isolates pairing status polling from pairing mutations and general API traffic', () => {
    const secret = 'a-secret-with-at-least-32-characters';
    const ip = '203.0.113.10';
    const statusPath =
      '/v1/pairing-sessions/018f4c7b-1ad6-7c95-bf34-5e45881f98a1/status';
    const statusKey = rateLimitKey(secret, ip, statusPath, 123, 'GET');
    const createKey = rateLimitKey(
      secret,
      ip,
      '/v1/pairing-sessions',
      123,
      'POST',
    );
    const apiKey = rateLimitKey(secret, ip, '/v1/rooms', 123, 'GET');

    expect(statusKey).toMatch(/^rate-limit:pairing-status:123:[a-f0-9]{64}$/);
    expect(statusKey).not.toBe(createKey);
    expect(statusKey).not.toBe(apiKey);
    expect(createKey).toMatch(/^rate-limit:pairing:123:[a-f0-9]{64}$/);
    expect(apiKey).toMatch(/^rate-limit:api:123:[a-f0-9]{64}$/);
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
