import { BadRequestException } from '@nestjs/common';
import { requireIdempotencyKey } from './idempotency-key';

describe('requireIdempotencyKey', () => {
  it('returns a bounded retry key', () => {
    expect(requireIdempotencyKey('disconnect-attempt-1')).toBe(
      'disconnect-attempt-1',
    );
  });

  it.each([undefined, '', 'short'])('rejects an invalid key: %p', (value) => {
    expect(() => requireIdempotencyKey(value)).toThrow(BadRequestException);
  });
});
