import type { ErrorEvent } from '@sentry/nestjs';
import { initializeSentry, scrubSentryEvent } from './sentry';

describe('Sentry privacy boundary', () => {
  it('stays disabled without a DSN and rejects invalid configuration', () => {
    expect(initializeSentry({})).toBe(false);
    expect(() => initializeSentry({ SENTRY_DSN: 'not-a-url' })).toThrow(
      'UNCONFIGURED: SENTRY_DSN',
    );
  });

  it('removes request data, tokens, and absolute paths', () => {
    const scrubbed = scrubSentryEvent({
      message: 'Bearer private-token at C:\\Users\\owner\\secret.txt',
      request: { url: 'https://example.test/file?token=secret' },
      user: { id: 'private-user' },
      extra: { content: 'private file content' },
      exception: {
        values: [
          {
            value: 'failed at /home/owner/private/file.txt',
            stacktrace: {
              frames: [
                {
                  filename: 'main.ts',
                  abs_path: '/opt/housemouse/apps/server/main.ts',
                  vars: { token: 'secret' },
                },
              ],
            },
          },
        ],
      },
    } as unknown as ErrorEvent);

    const encoded = JSON.stringify(scrubbed);
    expect(encoded).not.toContain('private-token');
    expect(encoded).not.toContain('secret.txt');
    expect(encoded).not.toContain('private file content');
    expect(encoded).not.toContain('/opt/housemouse');
    expect(scrubbed.message).toContain('[REDACTED_PATH]');
  });
});
