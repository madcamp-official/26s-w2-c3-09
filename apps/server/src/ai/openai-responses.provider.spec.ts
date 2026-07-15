import { OpenAiResponsesProvider } from './openai-responses.provider';

function providerWith(
  fetcher: ConstructorParameters<typeof OpenAiResponsesProvider>[0]['fetcher'],
) {
  return new OpenAiResponsesProvider({
    apiKey: 'test-openai-key',
    model: 'gpt-test',
    timeoutMs: 1000,
    maxOutputTokens: 500,
    endpoint: 'https://api.openai.test/v1/responses',
    fetcher,
  });
}

function okOutput(value: unknown) {
  return new Response(JSON.stringify({ output_text: JSON.stringify(value) }), {
    status: 200,
  });
}

describe('OpenAiResponsesProvider', () => {
  it('drafts commands through Responses API output then validates server contracts', async () => {
    const fetcher = jest.fn(async (_input: string | URL, _init?: RequestInit) =>
      okOutput({
        kind: 'COMMAND_DRAFT',
        reasonCode: '',
        reply: '',
        intent: 'RENAME',
        argumentsJson: JSON.stringify({
          rootId: 'root:downloads',
          sourceRelativePath: 'reports/old.pdf',
          newName: 'final.pdf',
        }),
        confirmationSummary: 'Rename reports/old.pdf to final.pdf',
      }),
    );
    const result = await providerWith(fetcher).classifyAndRespond({
      userId: 'user-1',
      roomId: 'room-1',
      sessionId: 'session-1',
      sourceMessage: {
        id: 'message-1',
        content: 'rename old report to final.pdf',
      },
    });

    expect(result).toEqual({
      status: 'READY',
      kind: 'COMMAND_DRAFT',
      command: {
        intent: 'RENAME',
        payload: {
          rootId: 'root:downloads',
          sourceRelativePath: 'reports/old.pdf',
          newName: 'final.pdf',
        },
      },
      confirmationSummary: 'Rename reports/old.pdf to final.pdf',
    });
    const init = fetcher.mock.calls[0][1] as RequestInit;
    expect(init.headers).toMatchObject({
      Authorization: 'Bearer test-openai-key',
      'Content-Type': 'application/json',
    });
    const requestBody = JSON.parse(init.body as string) as {
      text: {
        format: {
          schema: {
            properties: Record<string, unknown>;
            required: string[];
          };
        };
      };
    };
    expect(requestBody).toMatchObject({
      model: 'gpt-test',
      max_output_tokens: 500,
      text: {
        format: {
          type: 'json_schema',
          name: 'mousekeeper_command_draft',
          strict: true,
        },
      },
    });
    expect(new Set(requestBody.text.format.schema.required)).toEqual(
      new Set(Object.keys(requestBody.text.format.schema.properties)),
    );
    expect(JSON.parse(init.body as string).instructions).toContain(
      'Classification policy:',
    );
    expect(JSON.parse(init.body as string).instructions).toContain(
      'quick cleanup',
    );
    expect(JSON.parse(init.body as string).instructions).toContain(
      'sourceRelativePaths',
    );
    expect(JSON.parse(init.body as string).instructions).toContain(
      'createFileBrowseRequestSchema',
    );
    expect(JSON.parse(init.body as string).instructions).toContain(
      'destinationTemplate',
    );
  });

  it('rejects model output that fails command payload validation', async () => {
    const fetcher = jest.fn(async (_input: string | URL, _init?: RequestInit) =>
      okOutput({
        kind: 'COMMAND_DRAFT',
        reasonCode: '',
        reply: '',
        intent: 'RENAME',
        argumentsJson: JSON.stringify({
          rootId: 'root:downloads',
          sourceRelativePath: 'reports/old.pdf',
          newName: '../escape.pdf',
        }),
        confirmationSummary: 'Rename reports/old.pdf',
      }),
    );

    await expect(
      providerWith(fetcher).classifyAndRespond({
        userId: 'user-1',
        roomId: 'room-1',
        sessionId: 'session-1',
        sourceMessage: { id: 'message-1', content: 'rename it' },
      }),
    ).resolves.toEqual({
      status: 'INVALID',
      code: 'AI_OUTPUT_INVALID',
    });
  });

  it('preserves a validated conversational reply for chat persistence', async () => {
    const fetcher = jest.fn(async (_input: string | URL, _init?: RequestInit) =>
      okOutput({
        kind: 'NO_ACTION',
        reasonCode: '',
        reply: '네, MouseKeeper AI가 연결되어 있어요.',
        intent: 'NONE',
        argumentsJson: '{}',
        confirmationSummary: '',
      }),
    );

    await expect(
      providerWith(fetcher).classifyAndRespond({
        userId: 'user-1',
        roomId: 'room-1',
        sessionId: 'session-1',
        sourceMessage: { id: 'message-1', content: 'AI가 연결됐어?' },
      }),
    ).resolves.toEqual({
      status: 'READY',
      kind: 'NO_ACTION',
      reply: '네, MouseKeeper AI가 연결되어 있어요.',
    });
  });

  it('classifies chat rule requests into validated rule drafts', async () => {
    const fetcher = jest.fn(async (_input: string | URL, _init?: RequestInit) =>
      okOutput({
        kind: 'RULE_DRAFT',
        reasonCode: '',
        reply: '',
        intent: 'NONE',
        argumentsJson: '{}',
        confirmationSummary: '',
        name: 'PDF archive',
        definitionJson: JSON.stringify({
          match: 'ALL',
          conditions: [{ field: 'extension', operator: 'IN', value: ['.pdf'] }],
          action: { type: 'MOVE', destinationTemplate: 'Archive/PDF' },
        }),
        explanation: 'Move PDF files into Archive/PDF.',
        ambiguities: [],
      }),
    );

    await expect(
      providerWith(fetcher).classifyAndRespond({
        userId: 'user-1',
        roomId: 'room-1',
        sessionId: 'session-1',
        sourceMessage: {
          id: 'message-1',
          content: 'make a rule that moves PDFs to Archive/PDF',
        },
      }),
    ).resolves.toEqual({
      status: 'READY',
      kind: 'RULE_DRAFT',
      draft: {
        name: 'PDF archive',
        definition: {
          match: 'ALL',
          conditions: [{ field: 'extension', operator: 'IN', value: ['.pdf'] }],
          action: { type: 'MOVE', destinationTemplate: 'Archive/PDF' },
        },
        explanation: 'Move PDF files into Archive/PDF.',
        ambiguities: [],
      },
    });
  });

  it('classifies file lookups into validated query requests', async () => {
    const fetcher = jest.fn(async (_input: string | URL, _init?: RequestInit) =>
      okOutput({
        kind: 'QUERY',
        reasonCode: '',
        reply: '',
        intent: 'NONE',
        argumentsJson: '{}',
        confirmationSummary: '',
        browseJson: JSON.stringify({
          relativeDirectory: 'Documents',
          cursor: null,
          query: 'report',
          extensions: ['.pdf'],
          limit: 25,
          searchScope: 'MANAGED_ROOT',
        }),
        responseSummary: 'Looking for report PDFs under Documents.',
      }),
    );

    await expect(
      providerWith(fetcher).classifyAndRespond({
        userId: 'user-1',
        roomId: 'room-1',
        sessionId: 'session-1',
        sourceMessage: {
          id: 'message-1',
          content: 'show me report PDFs under Documents',
        },
      }),
    ).resolves.toEqual({
      status: 'READY',
      kind: 'QUERY',
      browse: {
        relativeDirectory: 'Documents',
        cursor: null,
        query: 'report',
        extensions: ['.pdf'],
        limit: 25,
        searchScope: 'MANAGED_ROOT',
      },
      responseSummary: 'Looking for report PDFs under Documents.',
    });
  });

  it('translates rule drafts only after Rule DSL validation', async () => {
    const fetcher = jest.fn(async (_input: string | URL, _init?: RequestInit) =>
      okOutput({
        kind: 'RULE_DRAFT',
        reasonCode: '',
        reply: '',
        name: 'PDF archive',
        definitionJson: JSON.stringify({
          match: 'ALL',
          conditions: [{ field: 'extension', operator: 'IN', value: ['.pdf'] }],
          action: { type: 'MOVE', destinationTemplate: 'Archive/PDF' },
        }),
        explanation: 'Move PDF files into Archive/PDF.',
        ambiguities: [],
      }),
    );

    await expect(
      providerWith(fetcher).translateRule({
        userId: 'user-1',
        roomId: 'room-1',
        instruction: 'move pdfs to archive',
      }),
    ).resolves.toEqual({
      status: 'READY',
      kind: 'RULE_DRAFT',
      draft: {
        name: 'PDF archive',
        definition: {
          match: 'ALL',
          conditions: [{ field: 'extension', operator: 'IN', value: ['.pdf'] }],
          action: { type: 'MOVE', destinationTemplate: 'Archive/PDF' },
        },
        explanation: 'Move PDF files into Archive/PDF.',
        ambiguities: [],
      },
    });
  });

  it('returns UNCONFIGURED for missing or rejected provider credentials', async () => {
    const missing = new OpenAiResponsesProvider({
      apiKey: '',
      model: 'gpt-test',
      fetcher: jest.fn(),
    });
    await expect(
      missing.classifyAndRespond({
        userId: 'user-1',
        roomId: 'room-1',
        sessionId: 'session-1',
        sourceMessage: { id: 'message-1', content: 'hello' },
      }),
    ).resolves.toEqual({
      status: 'UNCONFIGURED',
      code: 'AI_PROVIDER_UNCONFIGURED',
    });

    const rejected = providerWith(
      jest.fn(async () => new Response('{}', { status: 401 })),
    );
    await expect(
      rejected.translateRule({
        userId: 'user-1',
        roomId: 'room-1',
        instruction: 'clean pdfs',
      }),
    ).resolves.toEqual({
      status: 'UNCONFIGURED',
      code: 'AI_PROVIDER_UNCONFIGURED',
    });
  });

  it('returns UNCONFIGURED when the configured model is not available to the provider', async () => {
    const unavailableModel = providerWith(
      jest.fn(
        async () =>
          new Response(
            JSON.stringify({
              error: { message: 'The model `gpt-5.4` does not exist.' },
            }),
            { status: 404 },
          ),
      ),
    );

    await expect(
      unavailableModel.classifyAndRespond({
        userId: 'user-1',
        roomId: 'room-1',
        sessionId: 'session-1',
        sourceMessage: { id: 'message-1', content: 'hello' },
      }),
    ).resolves.toEqual({
      status: 'UNCONFIGURED',
      code: 'AI_PROVIDER_UNCONFIGURED',
    });
  });
});
