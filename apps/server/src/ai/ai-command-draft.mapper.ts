import { createCommandDraftSchema } from '@mousekeeper/contracts';
import type { z } from 'zod';
import type {
  AiInvalidResult,
  AiNoActionResult,
  AiProviderResult,
  AiUnavailableResult,
} from './ai.provider';

export type AiNoDraftResult =
  AiUnavailableResult | AiInvalidResult | AiNoActionResult;

export type AiCommandDraftMapping =
  | {
      kind: 'NO_DRAFT';
      aiStatus: AiNoDraftResult['status'];
      ai: AiNoDraftResult;
    }
  | {
      kind: 'INVALID';
      aiStatus: 'INVALID';
      ai: AiInvalidResult;
    }
  | {
      kind: 'COMMAND_DRAFT';
      aiStatus: 'READY';
      draftInput: z.infer<typeof createCommandDraftSchema>;
    };

export function mapAiResultToCommandDraft(
  sourceMessageId: string,
  ai: AiProviderResult,
): AiCommandDraftMapping {
  if (ai.status === 'UNCONFIGURED' || ai.status === 'INVALID') {
    return { kind: 'NO_DRAFT', aiStatus: ai.status, ai };
  }
  if (ai.kind === 'NO_ACTION') {
    return { kind: 'NO_DRAFT', aiStatus: ai.status, ai };
  }
  if (ai.kind !== 'COMMAND_DRAFT') {
    return {
      kind: 'INVALID',
      aiStatus: 'INVALID',
      ai: invalidAiOutput(),
    };
  }

  const draftInput = createCommandDraftSchema.safeParse({
    sourceMessageId,
    command: ai.command,
    confirmationSummary: ai.confirmationSummary,
    expiresAt: ai.expiresAt,
  });
  if (!draftInput.success) {
    return {
      kind: 'INVALID',
      aiStatus: 'INVALID',
      ai: invalidAiOutput(),
    };
  }
  return {
    kind: 'COMMAND_DRAFT',
    aiStatus: 'READY',
    draftInput: draftInput.data,
  };
}

export function invalidAiOutput(): AiInvalidResult {
  return {
    status: 'INVALID',
    code: 'AI_OUTPUT_INVALID',
  };
}
