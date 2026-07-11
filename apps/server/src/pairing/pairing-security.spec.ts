import { hashPairingCode } from './pairing.service';

describe('pairing code storage', () => {
  it('uses a server-secret HMAC rather than storing or directly hashing the code', () => {
    const first = hashPairingCode(
      '123456',
      'first-server-secret-at-least-32-bytes',
    );
    const second = hashPairingCode(
      '123456',
      'second-server-secret-at-least-32-byte',
    );
    expect(first).toMatch(/^[a-f0-9]{64}$/);
    expect(first).not.toContain('123456');
    expect(first).not.toBe(second);
  });
});
