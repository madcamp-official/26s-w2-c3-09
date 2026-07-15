import type {
  createCommandSchema,
  createFileBrowseRequestSchema,
  ruleDefinitionSchema,
} from '@mousekeeper/contracts';
import type { z } from 'zod';

export const AI_PROVIDER = Symbol('AI_PROVIDER');

/**
 * Cheap, best-effort room context the server already has in its own DB -
 * NOT a live snapshot of the managed root's actual folder/file state (the
 * server has no synchronous way to ask desktop for that; see the rule
 * preview dry-run gap). Existing rule destinations are useful precedent for
 * "does a folder like this already exist / get used" without claiming to
 * know the filesystem.
 */
export type RoomContext = {
  roomName: string;
  rootAlias: string;
  aiDocumentAnalysisConsent?: boolean;
  existingRules: { name: string; destinationTemplate: string | null }[];
};

export type FileContext = {
  source: 'SERVER_CACHE';
  isLiveFilesystemSnapshot: false;
  generatedAt: string;
  topLevelFolders: string[];
  knownFolders: string[];
  extensionDistribution: { extension: string; count: number }[];
  recentFiles: {
    relativePath: string;
    extension: string | null;
    sizeBytes: number;
    modifiedAt: string | null;
    cachedAt: string;
  }[];
  latestBrowse:
    | {
        relativeDirectory: string;
        status: string;
        requestedAt: string;
        directories: string[];
        files: string[];
      }
    | null;
  latestSnapshot:
    | {
        score: number;
        metrics: unknown;
        calculatedAt: string;
      }
    | null;
  recentProposals: {
    status: string;
    summary: unknown;
    createdAt: string;
    itemCount: number;
    sampleItems: {
      actionType: string;
      sourceRelativePath: string | null;
      destinationRelativePath: string | null;
    }[];
  }[];
};

export type ChatContext = {
  userId: string;
  roomId: string;
  sessionId: string;
  sourceMessage: {
    id: string;
    content: string;
  };
  room?: RoomContext | null;
  fileContext?: FileContext | null;
  documentChunks?: { relativePath: string; chunks: string[]; modifiedUnixMs: number } | null;
};

export type RuleTranslationContext = {
  userId: string;
  roomId: string;
  instruction: string;
  room?: RoomContext | null;
  fileContext?: FileContext | null;
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
