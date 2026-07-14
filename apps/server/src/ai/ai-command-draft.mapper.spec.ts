import { mapAiResultToCommandDraft } from './ai-command-draft.mapper';
import type { AiProviderResult } from './ai.provider';

const sourceMessageId = '018f4c7b-1ad6-7c95-bf34-5e45881f98a1';

describe('mapAiResultToCommandDraft', () => {
  it('keeps unconfigured AI as a no-draft result', () => {
    expect(
      mapAiResultToCommandDraft(sourceMessageId, {
        status: 'UNCONFIGURED',
        code: 'AI_PROVIDER_UNCONFIGURED',
      }),
    ).toEqual({
      kind: 'NO_DRAFT',
      aiStatus: 'UNCONFIGURED',
      ai: {
        status: 'UNCONFIGURED',
        code: 'AI_PROVIDER_UNCONFIGURED',
      },
    });
  });

  it('keeps no-action AI output out of command drafts', () => {
    expect(
      mapAiResultToCommandDraft(sourceMessageId, {
        status: 'READY',
        kind: 'NO_ACTION',
        reply: 'How can I help with your managed folder?',
      }),
    ).toEqual({
      kind: 'NO_DRAFT',
      aiStatus: 'READY',
      ai: {
        status: 'READY',
        kind: 'NO_ACTION',
        reply: 'How can I help with your managed folder?',
      },
    });
  });

  it('maps validated command draft output to the server draft contract', () => {
    const command: AiProviderResult = {
      status: 'READY',
      kind: 'COMMAND_DRAFT',
      command: {
        intent: 'RENAME',
        payload: {
          rootId: 'root-1',
          sourceRelativePath: 'reports/old.pdf',
          newName: 'final.pdf',
        },
      },
      confirmationSummary: 'Rename reports/old.pdf to final.pdf',
      expiresAt: '2026-07-14T01:02:03.000Z',
    };

    expect(mapAiResultToCommandDraft(sourceMessageId, command)).toEqual({
      kind: 'COMMAND_DRAFT',
      aiStatus: 'READY',
      draftInput: {
        sourceMessageId,
        command: command.command,
        confirmationSummary: command.confirmationSummary,
        expiresAt: command.expiresAt,
      },
    });
  });

  it('rejects AI attempts to inject server-owned command metadata', () => {
    const result = mapAiResultToCommandDraft(sourceMessageId, {
      status: 'READY',
      kind: 'COMMAND_DRAFT',
      command: {
        intent: 'RENAME',
        payload: {
          rootId: 'root-1',
          sourceRelativePath: 'reports/old.pdf',
          newName: 'final.pdf',
        },
        metadata: {
          sessionId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a2',
          requiresApproval: false,
        },
      },
      confirmationSummary: 'Rename reports/old.pdf to final.pdf',
    });

    expect(result).toEqual({
      kind: 'INVALID',
      aiStatus: 'INVALID',
      ai: {
        status: 'INVALID',
        code: 'AI_OUTPUT_INVALID',
      },
    });
  });

  it('rejects invalid draft fields before they enter product logic', () => {
    const result = mapAiResultToCommandDraft(sourceMessageId, {
      status: 'READY',
      kind: 'COMMAND_DRAFT',
      command: {
        intent: 'RENAME',
        payload: {
          rootId: 'root-1',
          sourceRelativePath: 'reports/old.pdf',
          newName: 'final.pdf',
        },
      },
      confirmationSummary: 'Rename reports/old.pdf to final.pdf',
      expiresAt: 'not-a-date',
    });

    expect(result).toEqual({
      kind: 'INVALID',
      aiStatus: 'INVALID',
      ai: {
        status: 'INVALID',
        code: 'AI_OUTPUT_INVALID',
      },
    });
  });
});
