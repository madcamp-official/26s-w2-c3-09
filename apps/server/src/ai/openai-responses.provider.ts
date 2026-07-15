import {
  commandIntentSchema,
  createCommandSchema,
  createFileBrowseRequestSchema,
  ruleDefinitionSchema,
} from '@mousekeeper/contracts';
import { Logger } from '@nestjs/common';
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
  classifierModel?: string;
  timeoutMs?: number;
  maxOutputTokens?: number;
  endpoint?: string;
  fetcher?: Fetcher;
};

type StructuredCallResult =
  | { status: 'OK'; value: unknown }
  | { status: 'UNCONFIGURED' }
  | { status: 'INVALID'; reason: string };

const DEFAULT_ENDPOINT = 'https://api.openai.com/v1/responses';
const DEFAULT_TIMEOUT_MS = 30_000;
const DEFAULT_MAX_OUTPUT_TOKENS = 3_000;
const INCOMPLETE_RETRY_MULTIPLIER = 2;
const classifierJsonSchema = {
  type: 'object',
  additionalProperties: false,
  required: ['route', 'needsTools'],
  properties: {
    route: {
      type: 'string',
      enum: [
        'CHAT',
        'QUERY',
        'COMMAND',
        'RULE',
        'HISTORY',
        'TRANSFER',
        'UNDO',
        'REFUSE',
      ],
    },
    needsTools: { type: 'boolean' },
  },
} as const;
const classifierResultSchema = z
  .object({
    route: z.enum([
      'CHAT',
      'QUERY',
      'COMMAND',
      'RULE',
      'HISTORY',
      'TRANSFER',
      'UNDO',
      'REFUSE',
    ]),
    needsTools: z.boolean(),
  })
  .strict();

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
    arguments: z.record(z.string(), z.unknown()).optional(),
    argumentsJson: z.string().max(20_000).optional(),
    confirmationSummary: z.string().max(1000),
    browse: z.record(z.string(), z.unknown()).optional(),
    browseJson: z.string().max(20_000).optional(),
    responseSummary: z.string().max(1000).optional(),
    name: z.string().max(120).optional(),
    definition: z.record(z.string(), z.unknown()).optional(),
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
    definition: z.record(z.string(), z.unknown()).optional(),
    definitionJson: z.string().max(20_000).optional(),
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
    'arguments',
    'confirmationSummary',
    'browse',
    'responseSummary',
    'name',
    'definition',
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
    arguments: { type: 'object', additionalProperties: true },
    confirmationSummary: { type: 'string' },
    browse: { type: 'object', additionalProperties: true },
    responseSummary: { type: 'string' },
    name: { type: 'string' },
    definition: { type: 'object', additionalProperties: true },
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
    'definition',
    'explanation',
    'ambiguities',
  ],
  properties: {
    kind: { type: 'string', enum: ['REFUSE', 'RULE_DRAFT'] },
    reasonCode: { type: 'string' },
    reply: { type: 'string' },
    name: { type: 'string' },
    definition: { type: 'object', additionalProperties: true },
    explanation: { type: 'string' },
    ambiguities: {
      type: 'array',
      items: { type: 'string' },
    },
  },
} as const;

export class OpenAiResponsesProvider implements AiProvider {
  private readonly logger = new Logger(OpenAiResponsesProvider.name);
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
    const classification = await this.classify(input);
    if (classification?.status === 'INVALID') {
      return this.invalid(classification.reason);
    }
    if (classification?.status === 'UNCONFIGURED') return this.unconfigured();
    const result = await this.callStructured({
      name: 'mousekeeper_command_draft',
      schema: commandDraftJsonSchema,
      instructions: commandInstructions(),
      input: JSON.stringify({
        roomId: input.roomId,
        sessionId: input.sessionId,
        sourceMessageId: input.sourceMessage.id,
        userMessage: input.sourceMessage.content,
        classification:
          classification?.status === 'OK' ? classification.value : null,
        roomContext: input.room,
        fileContext: input.fileContext,
        documentChunks: input.documentChunks,
      }),
    });
    if (result.status === 'UNCONFIGURED') return this.unconfigured();
    if (result.status === 'INVALID') return this.invalid(result.reason);

    const parsed = rawCommandDraftSchema.safeParse(result.value);
    if (!parsed.success) return this.invalid('COMMAND_ENVELOPE_SCHEMA');
    if (parsed.data.kind === 'RULE_DRAFT') {
      if (
        !parsed.data.name?.trim() ||
        !parsed.data.explanation?.trim() ||
        (parsed.data.definition == null && parsed.data.definitionJson == null)
      ) {
        return this.invalid('RULE_DRAFT_FIELDS');
      }
      const definition = ruleDefinitionSchema.safeParse(
        objectField(parsed.data.definition, parsed.data.definitionJson),
      );
      if (!definition.success) return this.invalid('RULE_DEFINITION_SCHEMA');
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
      if (
        (!parsed.data.browse && !parsed.data.browseJson) ||
        !parsed.data.responseSummary?.trim()
      ) {
        return this.invalid('QUERY_FIELDS');
      }
      const browse = createFileBrowseRequestSchema.safeParse(
        objectField(parsed.data.browse, parsed.data.browseJson),
      );
      if (!browse.success) return this.invalid('QUERY_BROWSE_SCHEMA');
      return {
        status: 'READY',
        kind: 'QUERY',
        browse: browse.data,
        responseSummary: parsed.data.responseSummary,
      };
    }
    if (parsed.data.kind !== 'COMMAND_DRAFT') {
      const reply = parsed.data.reply.trim();
      if (reply === '') return this.invalid('EMPTY_REPLY');
      return { status: 'READY', kind: 'NO_ACTION', reply };
    }
    if (
      parsed.data.intent === 'NONE' ||
      parsed.data.confirmationSummary.trim() === ''
    ) {
      return this.invalid('COMMAND_FIELDS');
    }
    const argumentsObject = objectField(
      parsed.data.arguments,
      parsed.data.argumentsJson,
    );
    if (argumentsObject == null) return this.invalid('COMMAND_ARGUMENTS_JSON');

    const command = createCommandSchema.safeParse({
      intent: parsed.data.intent,
      payload: argumentsObject,
    });
    if (!command.success) return this.invalid('COMMAND_PAYLOAD_SCHEMA');
    if (
      command.data.intent === 'ORGANIZE' &&
      command.data.payload.ruleDraft == null
    ) {
      return this.invalid('ORGANIZE_RULE_DRAFT_REQUIRED');
    }
    return {
      status: 'READY',
      kind: 'COMMAND_DRAFT',
      command: command.data,
      confirmationSummary: parsed.data.confirmationSummary,
    };
  }

  private async classify(input: ChatContext) {
    const model = this.options.classifierModel?.trim();
    if (!model) return null;
    const result = await this.callStructured({
      name: 'mousekeeper_route',
      model,
      schema: classifierJsonSchema,
      instructions:
        'Classify the latest request. Return CHAT, QUERY, COMMAND, RULE, HISTORY, TRANSFER, UNDO, or REFUSE. needsTools is true when factual room/file state must be inspected.',
      input: JSON.stringify({ userMessage: input.sourceMessage.content }),
    });
    if (result.status !== 'OK') return result;
    const parsed = classifierResultSchema.safeParse(result.value);
    return parsed.success
      ? ({ status: 'OK', value: parsed.data } as const)
      : ({ status: 'INVALID', reason: 'CLASSIFIER_SCHEMA' } as const);
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
    if (result.status === 'INVALID') return this.invalid(result.reason);

    const parsed = rawRuleDraftSchema.safeParse(result.value);
    if (!parsed.success || parsed.data.kind !== 'RULE_DRAFT') {
      return this.invalid('RULE_ENVELOPE_SCHEMA');
    }
    if (
      parsed.data.name.trim() === '' ||
      parsed.data.explanation.trim() === ''
    ) {
      return this.invalid('RULE_DRAFT_FIELDS');
    }
    const definition = ruleDefinitionSchema.safeParse(
      objectField(parsed.data.definition, parsed.data.definitionJson),
    );
    if (!definition.success) return this.invalid('RULE_DEFINITION_SCHEMA');
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
    model?: string;
    schema: unknown;
    instructions: string;
    input: string;
  }): Promise<StructuredCallResult> {
    const first = await this.callStructuredOnce(input, this.maxOutputTokens);
    if (first.status !== 'INVALID' || first.reason !== 'INCOMPLETE_OUTPUT') {
      return first;
    }
    this.logger.warn('AI structured output was incomplete; retrying once');
    return this.callStructuredOnce(
      input,
      this.maxOutputTokens * INCOMPLETE_RETRY_MULTIPLIER,
    );
  }

  private async callStructuredOnce(
    input: {
      name: string;
      model?: string;
      schema: unknown;
      instructions: string;
      input: string;
    },
    maxOutputTokens: number,
  ): Promise<StructuredCallResult> {
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
          model: input.model ?? this.options.model,
          instructions: input.instructions,
          input: input.input,
          max_output_tokens: maxOutputTokens,
          text: {
            format: {
              type: 'json_schema',
              name: input.name,
              // Nested command/rule DTOs are revalidated by authoritative Zod
              // contracts below, so the provider schema can carry direct objects.
              strict: false,
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
      if (text == null) {
        return {
          status: 'INVALID',
          reason: responseIncomplete(body)
            ? 'INCOMPLETE_OUTPUT'
            : 'MISSING_OUTPUT_TEXT',
        };
      }
      try {
        return { status: 'OK', value: JSON.parse(text) };
      } catch {
        return { status: 'INVALID', reason: 'OUTPUT_JSON_PARSE' };
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

  private invalid(reason = 'UNKNOWN') {
    // Do not log model output because it may contain user file names or paths.
    this.logger.warn(`Rejected AI output at validation stage: ${reason}`);
    return {
      status: 'INVALID' as const,
      code: 'AI_OUTPUT_INVALID' as const,
    };
  }
}

function responseIncomplete(value: unknown): boolean {
  if (value == null || typeof value !== 'object') return false;
  const record = value as Record<string, unknown>;
  if (record.status === 'incomplete') return true;
  const details = record.incomplete_details;
  return (
    details != null &&
    typeof details === 'object' &&
    (details as Record<string, unknown>).reason === 'max_output_tokens'
  );
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

function objectField(
  direct: Record<string, unknown> | undefined,
  legacy: string | undefined,
): Record<string, unknown> | null {
  return direct ?? (legacy == null ? null : parseJsonObject(legacy));
}

function commandInstructions() {
  return [
    'You are MouseKeeper chat intent classification and drafting logic.',
    'Return only the requested JSON object.',
    'Do not execute files, do not invent local file contents, and do not include command metadata.',
    'Always include every schema field; use empty strings, intent NONE, empty objects, and [] for irrelevant fields.',
    "Voice: you play a small, friendly house-mouse mascot who tidies the user's files.",
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
    '- A bulk request that selects files by a property or pattern and changes them is ORGANIZE, even when the user says "옮겨/move". MOVE is only for an explicit list of concrete relative paths.',
    '- "폴더를 만들고 조건에 맞는 파일을 그 안으로 옮겨줘" is one ORGANIZE draft with a MOVE action. The approved Desktop executor creates the destination directory safely; do not split it into an unrelated CREATE draft.',
    '- If a request both defines a rule and asks for an action: prefer RULE_DRAFT when they say to remember/automate it, COMMAND_DRAFT when they want it done once now.',
    '- When genuinely torn between two kinds, pick the least destructive (QUERY over COMMAND_DRAFT, NO_ACTION over a wrong COMMAND_DRAFT) and record the doubt in ambiguities.',
    'Never emit COMMAND_DRAFT for a write unless the intent is explicit enough for a later approval card; otherwise return NO_ACTION.',
    'Payload contract examples:',
    '- TRASH: arguments {"rootId":"root:<alias>","sourceRelativePaths":["relative/file.ext"]}. Use only user-provided relative paths. If no concrete path is given, return NO_ACTION and ask which file.',
    '- MOVE: arguments {"rootId":"root:<alias>","sourceRelativePaths":["relative/file.ext"],"destinationRelativeDirectory":"Archive"}.',
    '- CREATE folder: arguments {"rootId":"root:<alias>","kind":"DIRECTORY","relativePath":"New Folder"}. CREATE file uses kind "FILE".',
    '- ORGANIZE: arguments must include {"rootId":"root:<alias>","scopeRelativePath":"","instruction":"the user request","ruleDraft":<RuleDefinition>}. ruleDraft is required because Desktop never interprets free-form instruction text.',
    '- Build every ORGANIZE ruleDraft from the current request; never hardcode a brand, prefix, extension, or destination. Supported conditions are extension IN, name CONTAINS/STARTS_WITH/ENDS_WITH, modifiedAgeDays or createdAgeDays GTE/GT, ageDays GTE, sizeBytes GTE/LTE, relativePath STARTS_WITH, and fileKind EQ. Combine them with match ALL or ANY.',
    '- Supported ORGANIZE actions are MOVE with destinationTemplate, TRASH, QUARANTINE, and CREATE_DIR. Prefer MOVE when the request both names a destination folder and selects files; the folder may be absent and will be created only after approval.',
    '- For a one-off set operation, translate the selector and destination into ruleDraft. Example: "KakaoTalk으로 시작하는 이미지들은 모두 카카오톡 이미지 폴더로 옮겨줘" => {"rootId":"root:<alias>","scopeRelativePath":"","instruction":"KakaoTalk으로 시작하는 이미지들은 모두 카카오톡 이미지 폴더로 옮겨줘","ruleDraft":{"match":"ALL","conditions":[{"field":"name","operator":"STARTS_WITH","value":"KakaoTalk"},{"field":"extension","operator":"IN","value":[".jpg",".jpeg",".png",".gif",".webp",".heic"]}],"action":{"type":"MOVE","destinationTemplate":"카카오톡 이미지"}}}.',
    '- Another valid one-off example is "30일 넘고 10MB 이상인 PDF를 오래된 문서로 정리해줘": match ALL with extension IN [".pdf"], modifiedAgeDays GTE 30, sizeBytes GTE 10485760, and MOVE destinationTemplate "오래된 문서".',
    '- ANALYZE: arguments {}.',
    'When roomContext.rootAlias is present, use it exactly as rootId. Only fall back to "root:downloads" when no room rootAlias is available. Never invent absolute local paths.',
    'For QUERY, put a createFileBrowseRequestSchema object in browse: {"relativeDirectory":"","cursor":null,"query":"docs","extensions":[],"limit":25,"searchScope":"MANAGED_ROOT"}. Use searchScope MANAGED_ROOT unless the user explicitly asks only for the current folder.',
    'For listing a named folder such as "img 폴더의 목록을 줘", use kind QUERY with relativeDirectory "img", query null, extensions [], and searchScope CURRENT_DIRECTORY.',
    'For RULE_DRAFT, put the Rule DSL object in definition and leave intent NONE, arguments {}, confirmationSummary empty, and browse {}.',
    'For COMMAND_DRAFT ANALYZE, use intent ANALYZE, arguments {}, and a confirmationSummary that says the PC will analyze and propose cleanup, not execute changes.',
    'Valid command intents are the MouseKeeper server contract intents.',
    "The input includes an optional roomContext: {roomName, rootAlias, existingRules: [{name, destinationTemplate}]}. It is NOT a live filesystem listing - never claim to know what files or folders currently exist from it. Use it only to: prefer the room's own rootAlias as the default rootId when set; recognize when a new RULE_DRAFT duplicates or conflicts with an existingRules entry (mention this briefly in explanation/ambiguities rather than silently proceeding); and reuse an existing rule's destinationTemplate wording when the user clearly means the same folder.",
    'The input may include fileContext from the server cache: {topLevelFolders, knownFolders, extensionDistribution, recentFiles, latestBrowse, latestSnapshot, recentProposals}. It is not live filesystem truth. Treat it as stale/partial evidence from cached files, completed browse results, cleanliness snapshots, and proposals. Use it to choose sensible defaults, mention likely existing folders, prefer destinationTemplate names that match knownFolders, and avoid redundant cleanup suggestions. Never claim a folder/file definitely exists unless the user just supplied it; say "I see it in the recent server cache" or draft the rule normally.',
    'If fileContext.latestBrowse is READY and matches the requested relativeDirectory, do not issue another QUERY. Return NO_ACTION with a concise Korean reply that summarizes its directories and files and states that the listing came from the completed desktop browse.',
    "A rule's destinationTemplate folder does not need to already exist - never refuse or ask a clarification question just because you cannot confirm a destination folder exists. Only ask a clarification question for genuinely missing information (which file, which condition, which destination) as instructed above.",
  ].join('\n');
}

function ruleInstructions() {
  return [
    'You are MouseKeeper rule drafting logic.',
    'Return only the requested JSON object.',
    'Create deterministic Rule DSL only; never claim files were changed or previewed.',
    'Always include every schema field; use empty strings, definition {}, and [] for irrelevant fields.',
    'Voice: you play a small, friendly house-mouse mascot. Write the `reply` field in Korean with a light squeaky-mouse flavor (an occasional soft "찍!"/"찍찍"), warm and short, and drop it for refusals. Keep name and explanation plain and precise.',
    'If the request lacks a clear condition or action, return REFUSE.',
    'Use only the MouseKeeper rule definition fields allowed by the server contract.',
    'Rule DSL examples: move PDFs => {"match":"ALL","conditions":[{"field":"extension","operator":"IN","value":[".pdf"]}],"action":{"type":"MOVE","destinationTemplate":"Archive/PDF"}}; trash temp files => {"match":"ALL","conditions":[{"field":"name","operator":"ENDS_WITH","value":".tmp"}],"action":{"type":"TRASH"}}.',
    'The input includes an optional roomContext: {roomName, rootAlias, existingRules: [{name, destinationTemplate}]}. It is not a live filesystem listing. Use existingRules only to avoid drafting an obvious duplicate of an existing rule and to reuse consistent destinationTemplate wording; note a likely duplicate briefly in explanation rather than refusing.',
    'The input may include fileContext from the server cache: topLevelFolders/knownFolders, extensionDistribution, recentFiles, latestBrowse, latestSnapshot, and recentProposals. It is stale/partial, not live filesystem truth. Use it to infer common extensions, likely destination folder names, and whether a requested destination appears in recent cache/browse data. Do not require the destination folder to appear there.',
    'A destinationTemplate folder does not need to already exist on disk - never refuse a rule just because you cannot confirm the destination folder exists.',
  ].join('\n');
}
