import {
  commandIntentSchema,
  createCommandSchema,
  createFileBrowseRequestSchema,
  ruleDefinitionSchema,
} from '@mousekeeper/contracts';
import { z } from 'zod';
import type {
  AiProvider,
  AiProviderResult,
  RuleDraftResult,
  ChatContext,
  RuleTranslationContext,
} from './ai.provider';

type Fetcher = (input: string | URL, init?: RequestInit) => Promise<Response>;

export type OpenAiResponsesProviderOptions = {
  apiKey: string;
  model: string;
  timeoutMs?: number;
  maxOutputTokens?: number;
  endpoint?: string;
  fetcher?: Fetcher;
};

type StructuredCallResult =
  | { status: 'OK'; value: unknown }
  | { status: 'UNCONFIGURED' }
  | { status: 'INVALID' };

const DEFAULT_ENDPOINT = 'https://api.openai.com/v1/responses';
const DEFAULT_TIMEOUT_MS = 30_000;
const DEFAULT_MAX_OUTPUT_TOKENS = 1_000;

const rawCommandDraftSchema = z
  .object({
    kind: z.enum([
      'NO_ACTION',
      'REFUSE',
      'COMMAND_DRAFT',
      'RULE_DRAFT',
      'QUERY',
    ]),
    reasonCode: z.string().max(100),
    reply: z.string().max(1000),
    intent: z.union([z.literal('NONE'), commandIntentSchema]),
    argumentsJson: z.string().max(20_000),
    confirmationSummary: z.string().max(1000),
    browseJson: z.string().max(20_000).optional(),
    responseSummary: z.string().max(1000).optional(),
    name: z.string().max(120).optional(),
    definitionJson: z.string().max(20_000).optional(),
    explanation: z.string().max(2000).optional(),
    ambiguities: z.array(z.string().max(300)).max(10).optional(),
  })
  .strict();

const rawRuleDraftSchema = z
  .object({
    kind: z.enum(['REFUSE', 'RULE_DRAFT']),
    reasonCode: z.string().max(100),
    reply: z.string().max(1000),
    name: z.string().max(120),
    definitionJson: z.string().max(20_000),
    explanation: z.string().max(2000),
    ambiguities: z.array(z.string().max(300)).max(10),
  })
  .strict();

const commandDraftJsonSchema = {
  type: 'object',
  additionalProperties: false,
  required: [
    'kind',
    'reasonCode',
    'reply',
    'intent',
    'argumentsJson',
    'confirmationSummary',
  ],
  properties: {
    kind: {
      type: 'string',
      enum: ['NO_ACTION', 'REFUSE', 'COMMAND_DRAFT', 'RULE_DRAFT', 'QUERY'],
    },
    reasonCode: { type: 'string' },
    reply: { type: 'string' },
    intent: {
      type: 'string',
      enum: ['NONE', ...commandIntentSchema.options],
    },
    argumentsJson: {
      type: 'string',
      description:
        'A JSON object string containing only the payload for the selected command intent. Use "{}" unless kind is COMMAND_DRAFT.',
    },
    confirmationSummary: { type: 'string' },
    browseJson: {
      type: 'string',
      description:
        'A JSON object string matching createFileBrowseRequestSchema. Use "{}" unless kind is QUERY.',
    },
    responseSummary: { type: 'string' },
    name: { type: 'string' },
    definitionJson: {
      type: 'string',
      description:
        'A JSON object string matching MouseKeeper Rule DSL. Use "{}" unless kind is RULE_DRAFT.',
    },
    explanation: { type: 'string' },
    ambiguities: {
      type: 'array',
      items: { type: 'string' },
    },
  },
} as const;

const ruleDraftJsonSchema = {
  type: 'object',
  additionalProperties: false,
  required: [
    'kind',
    'reasonCode',
    'reply',
    'name',
    'definitionJson',
    'explanation',
    'ambiguities',
  ],
  properties: {
    kind: { type: 'string', enum: ['REFUSE', 'RULE_DRAFT'] },
    reasonCode: { type: 'string' },
    reply: { type: 'string' },
    name: { type: 'string' },
    definitionJson: {
      type: 'string',
      description:
        'A JSON object string matching MouseKeeper Rule DSL. Use "{}" unless kind is RULE_DRAFT.',
    },
    explanation: { type: 'string' },
    ambiguities: {
      type: 'array',
      items: { type: 'string' },
    },
  },
} as const;

export class OpenAiResponsesProvider implements AiProvider {
  private readonly timeoutMs: number;
  private readonly maxOutputTokens: number;
  private readonly endpoint: string;
  private readonly fetcher: Fetcher;

  constructor(private readonly options: OpenAiResponsesProviderOptions) {
    this.timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
    this.maxOutputTokens = options.maxOutputTokens ?? DEFAULT_MAX_OUTPUT_TOKENS;
    this.endpoint = options.endpoint ?? DEFAULT_ENDPOINT;
    this.fetcher = options.fetcher ?? fetch;
  }

  async classifyAndRespond(input: ChatContext): Promise<AiProviderResult> {
    if (!this.isConfigured()) return this.unconfigured();
    const result = await this.callStructured({
      name: 'mousekeeper_command_draft',
      schema: commandDraftJsonSchema,
      instructions: commandInstructions(),
      input: JSON.stringify({
        roomId: input.roomId,
        sessionId: input.sessionId,
        sourceMessageId: input.sourceMessage.id,
        userMessage: input.sourceMessage.content,
      }),
    });
    if (result.status === 'UNCONFIGURED') return this.unconfigured();
    if (result.status === 'INVALID') return this.invalid();

    const parsed = rawCommandDraftSchema.safeParse(result.value);
    if (!parsed.success) return this.invalid();
    if (parsed.data.kind === 'RULE_DRAFT') {
      if (
        !parsed.data.name?.trim() ||
        !parsed.data.explanation?.trim() ||
        parsed.data.definitionJson == null
      ) {
        return this.invalid();
      }
      const definition = ruleDefinitionSchema.safeParse(
        parseJsonObject(parsed.data.definitionJson),
      );
      if (!definition.success) return this.invalid();
      return {
        status: 'READY',
        kind: 'RULE_DRAFT',
        draft: {
          name: parsed.data.name,
          definition: definition.data,
          explanation: parsed.data.explanation,
          ambiguities: parsed.data.ambiguities ?? [],
        },
      };
    }
    if (parsed.data.kind === 'QUERY') {
      if (!parsed.data.browseJson || !parsed.data.responseSummary?.trim()) {
        return this.invalid();
      }
      const browse = createFileBrowseRequestSchema.safeParse(
        parseJsonObject(parsed.data.browseJson),
      );
      if (!browse.success) return this.invalid();
      return {
        status: 'READY',
        kind: 'QUERY',
        browse: browse.data,
        responseSummary: parsed.data.responseSummary,
      };
    }
    if (parsed.data.kind !== 'COMMAND_DRAFT') {
      return { status: 'READY', kind: 'NO_ACTION' };
    }
    if (
      parsed.data.intent === 'NONE' ||
      parsed.data.confirmationSummary.trim() === ''
    ) {
      return this.invalid();
    }
    const argumentsObject = parseJsonObject(parsed.data.argumentsJson);
    if (argumentsObject == null) return this.invalid();

    const command = createCommandSchema.safeParse({
      intent: parsed.data.intent,
      payload: argumentsObject,
    });
    if (!command.success) return this.invalid();
    return {
      status: 'READY',
      kind: 'COMMAND_DRAFT',
      command: command.data,
      confirmationSummary: parsed.data.confirmationSummary,
    };
  }

  async translateRule(input: RuleTranslationContext): Promise<RuleDraftResult> {
    if (!this.isConfigured()) return this.unconfigured();
    const result = await this.callStructured({
      name: 'mousekeeper_rule_draft',
      schema: ruleDraftJsonSchema,
      instructions: ruleInstructions(),
      input: JSON.stringify({
        roomId: input.roomId,
        instruction: input.instruction,
      }),
    });
    if (result.status === 'UNCONFIGURED') return this.unconfigured();
    if (result.status === 'INVALID') return this.invalid();

    const parsed = rawRuleDraftSchema.safeParse(result.value);
    if (!parsed.success || parsed.data.kind !== 'RULE_DRAFT') {
      return this.invalid();
    }
    if (
      parsed.data.name.trim() === '' ||
      parsed.data.explanation.trim() === ''
    ) {
      return this.invalid();
    }
    const definition = ruleDefinitionSchema.safeParse(
      parseJsonObject(parsed.data.definitionJson),
    );
    if (!definition.success) return this.invalid();
    return {
      status: 'READY',
      kind: 'RULE_DRAFT',
      draft: {
        name: parsed.data.name,
        definition: definition.data,
        explanation: parsed.data.explanation,
        ambiguities: parsed.data.ambiguities,
      },
    };
  }

  private async callStructured(input: {
    name: string;
    schema: unknown;
    instructions: string;
    input: string;
  }): Promise<StructuredCallResult> {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.timeoutMs);
    try {
      const response = await this.fetcher(this.endpoint, {
        method: 'POST',
        headers: {
          Authorization: `Bearer ${this.options.apiKey}`,
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          model: this.options.model,
          instructions: input.instructions,
          input: input.input,
          max_output_tokens: this.maxOutputTokens,
          text: {
            format: {
              type: 'json_schema',
              name: input.name,
              strict: true,
              schema: input.schema,
            },
          },
        }),
        signal: controller.signal,
      });
      if (
        response.status === 400 ||
        response.status === 401 ||
        response.status === 403 ||
        response.status === 404
      ) {
        return { status: 'UNCONFIGURED' };
      }
      if (!response.ok) {
        throw new Error(`AI_PROVIDER_REQUEST_FAILED:${response.status}`);
      }
      const body = (await response.json()) as unknown;
      const text = responseText(body);
      if (text == null) return { status: 'INVALID' };
      try {
        return { status: 'OK', value: JSON.parse(text) };
      } catch {
        return { status: 'INVALID' };
      }
    } finally {
      clearTimeout(timer);
    }
  }

  private isConfigured() {
    return (
      this.options.apiKey.trim().length > 0 &&
      this.options.model.trim().length > 0
    );
  }

  private unconfigured() {
    return {
      status: 'UNCONFIGURED' as const,
      code: 'AI_PROVIDER_UNCONFIGURED' as const,
    };
  }

  private invalid() {
    return {
      status: 'INVALID' as const,
      code: 'AI_OUTPUT_INVALID' as const,
    };
  }
}

function responseText(value: unknown): string | null {
  if (value == null || typeof value !== 'object') return null;
  if ('output_text' in value && typeof value.output_text === 'string') {
    return value.output_text;
  }
  if (!('output' in value) || !Array.isArray(value.output)) return null;
  const parts: string[] = [];
  for (const item of value.output) {
    if (item == null || typeof item !== 'object') continue;
    if ('content' in item && Array.isArray(item.content)) {
      for (const content of item.content) {
        if (
          content != null &&
          typeof content === 'object' &&
          'text' in content &&
          typeof content.text === 'string'
        ) {
          parts.push(content.text);
        }
      }
    }
  }
  return parts.length > 0 ? parts.join('') : null;
}

function parseJsonObject(value: string): Record<string, unknown> | null {
  try {
    const parsed = JSON.parse(value) as unknown;
    return parsed != null &&
      typeof parsed === 'object' &&
      !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : null;
  } catch {
    return null;
  }
}

function commandInstructions() {
  return [
    'You are MouseKeeper chat intent classification and drafting logic.',
    'Return only the requested JSON object.',
    'Do not execute files, do not invent local file contents, and do not include command metadata.',
    'Always include every schema field; use empty strings, intent NONE, argumentsJson "{}", and [] for irrelevant fields.',
    'Classification policy:',
    '- General conversation, greetings, explanation requests, or unclear requests => NO_ACTION. Put a concise conversational reply in reply.',
    '- File-changing requests that can be expressed as a single approved MouseKeeper command => COMMAND_DRAFT.',
    '- File lookup, search, list, check, or "show me" requests that do not mutate files => QUERY.',
    '- Requests to add, remember, or automate a standing organization rule => RULE_DRAFT.',
    '- Requests like "quick cleanup", "clean this up for me", or "suggest cleanup" should become COMMAND_DRAFT with intent ANALYZE and payload {} unless the user asks only to view existing suggestions.',
    'Use COMMAND_DRAFT only when the user asks for a concrete managed-root file operation.',
    'Never use COMMAND_DRAFT for a write unless the user intent is explicit enough for a later approval card; otherwise return NO_ACTION.',
    'Use QUERY when the user asks to look up, search, list, or check files without changing files.',
    'For QUERY, put createFileBrowseRequestSchema JSON in browseJson. Use relativeDirectory "" for the managed root, cursor null, and searchScope MANAGED_ROOT unless the user names a subfolder.',
    'Use RULE_DRAFT only when the user asks to add, remember, or automate an ongoing file organization rule.',
    'For RULE_DRAFT, put the Rule DSL JSON in definitionJson and leave intent NONE, argumentsJson "{}", confirmationSummary empty, and browseJson "{}".',
    'For COMMAND_DRAFT ANALYZE, use intent ANALYZE, argumentsJson "{}", and a confirmationSummary that says the PC will analyze and propose cleanup, not execute changes.',
    'If the request is unsafe, outside managed roots, requests bypassing approval, or asks you to pretend success, return REFUSE.',
    'If the request is unclear or just conversation, return NO_ACTION.',
    'Valid command intents are the MouseKeeper server contract intents.',
  ].join('\n');
}

function ruleInstructions() {
  return [
    'You are MouseKeeper rule drafting logic.',
    'Return only the requested JSON object.',
    'Create deterministic Rule DSL only; never claim files were changed or previewed.',
    'Always include every schema field; use empty strings, definitionJson "{}", and [] for irrelevant fields.',
    'If the request lacks a clear condition or action, return REFUSE.',
    'Use only the MouseKeeper rule definition fields allowed by the server contract.',
  ].join('\n');
}
