import type {
  createCommandSchema,
  createFileBrowseRequestSchema,
  ruleDefinitionSchema,
} from '@mousekeeper/contracts';
import type { z } from 'zod';

export const AI_PROVIDER = Symbol('AI_PROVIDER');

export type ChatContext = {
  userId: string;
  roomId: string;
  sessionId: string;
  sourceMessage: {
    id: string;
    content: string;
  };
};

export type RuleTranslationContext = {
  userId: string;
  roomId: string;
  instruction: string;
};

export type AiUnavailableResult = {
  status: 'UNCONFIGURED';
  code: 'AI_PROVIDER_UNCONFIGURED';
};

export type AiInvalidResult = {
  status: 'INVALID';
  code: 'AI_OUTPUT_INVALID';
};

export type AiNoActionResult = {
  status: 'READY';
  kind: 'NO_ACTION';
  reply: string;
};

export type AiCommandDraftResult = {
  status: 'READY';
  kind: 'COMMAND_DRAFT';
  command: z.infer<typeof createCommandSchema>;
  confirmationSummary: string;
  expiresAt?: string;
};

export type AiQueryResult = {
  status: 'READY';
  kind: 'QUERY';
  browse: z.infer<typeof createFileBrowseRequestSchema>;
  responseSummary: string;
};

export type AiProviderResult =
  | AiUnavailableResult
  | AiInvalidResult
  | AiNoActionResult
  | AiCommandDraftResult
  | AiQueryResult
  | AiRuleDraftResult;

export type AiRuleDraftResult = {
  status: 'READY';
  kind: 'RULE_DRAFT';
  draft: {
    name: string;
    definition: z.infer<typeof ruleDefinitionSchema>;
    explanation: string;
    ambiguities: string[];
  };
};

export type RuleDraftResult =
  AiUnavailableResult | AiInvalidResult | AiRuleDraftResult;

export interface AiProvider {
  classifyAndRespond(input: ChatContext): Promise<AiProviderResult>;
  translateRule(input: RuleTranslationContext): Promise<RuleDraftResult>;
}
