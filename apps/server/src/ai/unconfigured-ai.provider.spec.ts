import { UnconfiguredAiProvider } from './unconfigured-ai.provider';

describe('UnconfiguredAiProvider', () => {
  it('returns an explicit unconfigured result without fabricating an assistant reply', async () => {
    const provider = new UnconfiguredAiProvider();

    await expect(
      provider.classifyAndRespond({
        userId: 'user-1',
        roomId: 'room-1',
        sessionId: 'session-1',
        sourceMessage: {
          id: 'message-1',
          content: 'Rename the report',
        },
      }),
    ).resolves.toEqual({
      status: 'UNCONFIGURED',
      code: 'AI_PROVIDER_UNCONFIGURED',
    });
  });

  it('keeps natural-language rule translation explicitly unconfigured', async () => {
    const provider = new UnconfiguredAiProvider();

    await expect(
      provider.translateRule({
        userId: 'user-1',
        roomId: 'room-1',
        instruction: 'Move old PDFs to archive',
      }),
    ).resolves.toEqual({
      status: 'UNCONFIGURED',
      code: 'AI_PROVIDER_UNCONFIGURED',
    });
  });
});
