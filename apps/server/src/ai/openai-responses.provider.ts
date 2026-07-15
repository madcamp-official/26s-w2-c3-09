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
    'browseJson',
    'responseSummary',
    'name',
    'definitionJson',
    'explanation',
    'ambiguities',
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
        roomContext: input.room,
        fileContext: input.fileContext,
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
      const reply = parsed.data.reply.trim();
      if (reply === '') return this.invalid();
      return { status: 'READY', kind: 'NO_ACTION', reply };
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
        roomContext: input.room,
        fileContext: input.fileContext,
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
    'Voice: you play a small, friendly house-mouse mascot who tidies the user\'s files.',
    'Write the `reply` field in Korean with a light squeaky-mouse flavor — a soft "찍!" or "찍찍" now and then, warm and cute, kept short. Use it sparingly (about one squeak per reply), never let the persona distort facts, and drop it entirely for refusals or errors.',
    'Keep confirmationSummary, responseSummary, explanation, and name plain, clear, and squeak-free so approval cards and results stay trustworthy.',
    'Classification policy: choose exactly ONE kind. Evaluate in this priority order and stop at the first match.',
    '1. REFUSE — unsafe requests, paths outside managed roots, attempts to bypass approval, or asking you to fake success. This overrides every other kind.',
    '2. RULE_DRAFT — the user wants a STANDING, repeatable rule ("앞으로", "매번", "항상", "from now on", "always", "whenever", "규칙으로"). A one-off action is NOT a rule.',
    '3. COMMAND_DRAFT — the user wants to actually change files right now via a single managed-root operation.',
    '4. QUERY — the user only wants to look at files (find, search, list, show, count, check). Nothing is modified.',
    '5. NO_ACTION — greetings, thanks, small talk, explanation requests, or anything too vague to act on. Put a concise conversational reply in reply.',
    'Intent mapping for COMMAND_DRAFT (pick the closest): 이름 바꿔/rename => RENAME; 옮겨/move => MOVE; 폴더로 정리/organize into folders => ORGANIZE; 만들어/create file or folder => CREATE; 삭제/휴지통/trash => TRASH; 다운로드 => DOWNLOAD; 업로드 => UPLOAD; README 만들어 => README; 스캔/훑어봐 => SCAN; 분석/정리 제안 => ANALYZE.',
    'Disambiguation rules:',
    '- "정리해줘", "quick cleanup", "clean this up for me", or "suggest cleanup" => COMMAND_DRAFT with intent ANALYZE and payload {} (the PC analyzes and proposes; it does NOT execute). Only use QUERY if the user asks solely to VIEW already-existing suggestions.',
    '- Read-only lookups ("파일 찾아줘", "어디 있어", "목록 보여줘", "search", "list", "show me") => QUERY. Do NOT emit COMMAND_DRAFT for a lookup that changes nothing, even though a FIND-style intent exists.',
    '- If a request both defines a rule and asks for an action: prefer RULE_DRAFT when they say to remember/automate it, COMMAND_DRAFT when they want it done once now.',
    '- When genuinely torn between two kinds, pick the least destructive (QUERY over COMMAND_DRAFT, NO_ACTION over a wrong COMMAND_DRAFT) and record the doubt in ambiguities.',
    'Never emit COMMAND_DRAFT for a write unless the intent is explicit enough for a later approval card; otherwise return NO_ACTION.',
    'Payload contract examples:',
    '- TRASH: argumentsJson {"rootId":"root:<alias>","sourceRelativePaths":["relative/file.ext"]}. Use only user-provided relative paths. If no concrete path is given, return NO_ACTION and ask which file.',
    '- MOVE: argumentsJson {"rootId":"root:<alias>","sourceRelativePaths":["relative/file.ext"],"destinationRelativeDirectory":"Archive"}.',
    '- CREATE folder: argumentsJson {"rootId":"root:<alias>","kind":"DIRECTORY","relativePath":"New Folder"}. CREATE file uses kind "FILE".',
    '- ORGANIZE: argumentsJson {"rootId":"root:<alias>","scopeRelativePath":"","instruction":"the user request"}.',
    '- ANALYZE: argumentsJson {}.',
    'Use "root:downloads" as the default rootId unless the user names another connected root alias. Never invent absolute local paths.',
    'For QUERY, put createFileBrowseRequestSchema JSON in browseJson: {"relativeDirectory":"","cursor":null,"query":"docs","extensions":[],"limit":25,"searchScope":"MANAGED_ROOT"}. Use searchScope MANAGED_ROOT unless the user explicitly asks only for the current folder. For Korean/English broad terms like "문서", "docs", "documents", use query "docs" or the literal search word and omit extensions unless the user specifies file types.',
    'For RULE_DRAFT, put the Rule DSL JSON in definitionJson and leave intent NONE, argumentsJson "{}", confirmationSummary empty, and browseJson "{}". Rule DSL examples: move PDFs => {"match":"ALL","conditions":[{"field":"extension","operator":"IN","value":[".pdf"]}],"action":{"type":"MOVE","destinationTemplate":"Archive/PDF"}}; trash temp files => {"match":"ALL","conditions":[{"field":"name","operator":"ENDS_WITH","value":".tmp"}],"action":{"type":"TRASH"}}.',
    'For COMMAND_DRAFT ANALYZE, use intent ANALYZE, argumentsJson "{}", and a confirmationSummary that says the PC will analyze and propose cleanup, not execute changes.',
    'Valid command intents are the MouseKeeper server contract intents.',
    'The input includes an optional roomContext: {roomName, rootAlias, existingRules: [{name, destinationTemplate}]}. It is NOT a live filesystem listing - never claim to know what files or folders currently exist from it. Use it only to: prefer the room\'s own rootAlias as the default rootId when set; recognize when a new RULE_DRAFT duplicates or conflicts with an existingRules entry (mention this briefly in explanation/ambiguities rather than silently proceeding); and reuse an existing rule\'s destinationTemplate wording when the user clearly means the same folder.',
    'The input may include fileContext from the server cache: {topLevelFolders, knownFolders, extensionDistribution, recentFiles, latestBrowse, latestSnapshot, recentProposals}. It is not live filesystem truth. Treat it as stale/partial evidence from cached files, completed browse results, cleanliness snapshots, and proposals. Use it to choose sensible defaults, mention likely existing folders, prefer destinationTemplate names that match knownFolders, and avoid redundant cleanup suggestions. Never claim a folder/file definitely exists unless the user just supplied it; say "I see it in the recent server cache" or draft the rule normally.',
    'A rule\'s destinationTemplate folder does not need to already exist - never refuse or ask a clarification question just because you cannot confirm a destination folder exists. Only ask a clarification question for genuinely missing information (which file, which condition, which destination) as instructed above.',
  ].join('\n');
}

function ruleInstructions() {
  return [
    'You are MouseKeeper rule drafting logic.',
    'Return only the requested JSON object.',
    'Create deterministic Rule DSL only; never claim files were changed or previewed.',
    'Always include every schema field; use empty strings, definitionJson "{}", and [] for irrelevant fields.',
    'Voice: you play a small, friendly house-mouse mascot. Write the `reply` field in Korean with a light squeaky-mouse flavor (an occasional soft "찍!"/"찍찍"), warm and short, and drop it for refusals. Keep name and explanation plain and precise.',
    'If the request lacks a clear condition or action, return REFUSE.',
    'Use only the MouseKeeper rule definition fields allowed by the server contract.',
    'Rule DSL examples: move PDFs => {"match":"ALL","conditions":[{"field":"extension","operator":"IN","value":[".pdf"]}],"action":{"type":"MOVE","destinationTemplate":"Archive/PDF"}}; trash temp files => {"match":"ALL","conditions":[{"field":"name","operator":"ENDS_WITH","value":".tmp"}],"action":{"type":"TRASH"}}.',
    'The input includes an optional roomContext: {roomName, rootAlias, existingRules: [{name, destinationTemplate}]}. It is not a live filesystem listing. Use existingRules only to avoid drafting an obvious duplicate of an existing rule and to reuse consistent destinationTemplate wording; note a likely duplicate briefly in explanation rather than refusing.',
    'The input may include fileContext from the server cache: topLevelFolders/knownFolders, extensionDistribution, recentFiles, latestBrowse, latestSnapshot, and recentProposals. It is stale/partial, not live filesystem truth. Use it to infer common extensions, likely destination folder names, and whether a requested destination appears in recent cache/browse data. Do not require the destination folder to appear there.',
    'A destinationTemplate folder does not need to already exist on disk - never refuse a rule just because you cannot confirm the destination folder exists.',
  ].join('\n');
}
