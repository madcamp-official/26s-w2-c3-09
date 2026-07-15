import {
  Body,
  BadRequestException,
  Controller,
  Delete,
  Get,
  Headers,
  Param,
  Patch,
  Post,
  Query,
  UseGuards,
} from '@nestjs/common';
import {
  chatMessagesQuerySchema,
  answerAgentClarificationSchema,
  confirmCommandDraftSchema,
  createChatMessageSchema,
  createChatSessionSchema,
  createCommandDraftSchema,
  markChatSessionReadSchema,
  quickCleanupSchema,
  rejectCommandDraftSchema,
  updateChatSessionSchema,
} from '@mousekeeper/contracts';
import { z } from 'zod';
import { CurrentPrincipal } from '../auth/auth-principal';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import { requireIdempotencyKey } from '../connections/idempotency-key';
import { ZodValidationPipe } from '../common/zod-validation.pipe';
import { ChatService } from './chat.service';
import { AgentRunsService } from './agent-runs.service';
import { AgentOnly } from '../auth/agent-only.decorator';
import { requireAgentDevice } from '../auth/agent-device';

@Controller('v1')
@UseGuards(FirebaseAuthGuard)
export class ChatController {
  constructor(
    private readonly chat: ChatService,
    private readonly agentRuns: AgentRunsService,
  ) {}

  @Get('agent-runs/:runId')
  getAgentRun(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('runId') runId: string,
  ) {
    return this.agentRuns.get(p.userId, runId);
  }

  @Post('agent-runs/:runId/clarifications')
  answerAgentClarification(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('runId') runId: string,
    @Body(new ZodValidationPipe(answerAgentClarificationSchema))
    body: z.infer<typeof answerAgentClarificationSchema>,
  ) {
    return this.agentRuns.answerClarification(
      p.userId,
      runId,
      body.selectedCandidate,
    );
  }

  @Get('devices/:deviceId/agent-tools/pending')
  @AgentOnly()
  pendingAgentTools(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('deviceId') deviceId: string,
  ) {
    requireAgentDevice(p, deviceId);
    return this.agentRuns.pendingTools(p.userId, deviceId);
  }

  @Post('agent/agent-tools/:stepId/result')
  @AgentOnly()
  completeAgentTool(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('stepId') stepId: string,
    @Body() body: unknown,
  ) {
    const deviceId = requireAgentDevice(p);
    const parsed = z.object({
      status: z.enum(['SUCCEEDED', 'FAILED']),
      resultMetadata: z.record(z.string(), z.unknown()).nullable().optional(),
      failureCode: z.string().trim().min(1).max(80).nullable().optional(),
    }).strict().safeParse(body);
    if (!parsed.success) throw new BadRequestException({ code: 'INVALID_AGENT_TOOL_RESULT' });
    const metadataBytes = Buffer.byteLength(JSON.stringify(parsed.data.resultMetadata ?? null), 'utf8');
    if (metadataBytes > 512_000) {
      throw new BadRequestException({ code: 'AGENT_TOOL_RESULT_TOO_LARGE' });
    }
    return this.agentRuns.completeTool(
      p.userId,
      deviceId,
      stepId,
      parsed.data.status,
      parsed.data.resultMetadata ?? null,
      parsed.data.failureCode ?? null,
    );
  }

  @Get('rooms/:roomId/chat-sessions')
  listSessions(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('roomId') roomId: string,
  ) {
    return this.chat.listSessions(p.userId, roomId);
  }

  @Get('rooms/:roomId/chat/quick-view')
  quickView(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('roomId') roomId: string,
  ) {
    return this.chat.quickView(p.userId, roomId);
  }

  @Post('rooms/:roomId/chat/quick-cleanup')
  quickCleanup(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('roomId') roomId: string,
    @Body(new ZodValidationPipe(quickCleanupSchema))
    _body: z.infer<typeof quickCleanupSchema>,
  ) {
    return this.chat.createQuickCleanupSuggestion(p.userId, roomId);
  }

  @Post('rooms/:roomId/chat-sessions')
  createSession(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('roomId') roomId: string,
    @Body(new ZodValidationPipe(createChatSessionSchema))
    body: z.infer<typeof createChatSessionSchema>,
  ) {
    return this.chat.createSession(p.userId, roomId, body.title);
  }

  @Patch('chat-sessions/:sessionId')
  updateSession(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('sessionId') sessionId: string,
    @Body(new ZodValidationPipe(updateChatSessionSchema))
    body: z.infer<typeof updateChatSessionSchema>,
  ) {
    return this.chat.updateSession(p.userId, sessionId, body.title!);
  }

  @Delete('chat-sessions/:sessionId')
  deleteSession(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('sessionId') sessionId: string,
  ) {
    return this.chat.deleteSession(p.userId, sessionId);
  }

  @Post('chat-sessions/:sessionId/read')
  markSessionRead(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('sessionId') sessionId: string,
    @Body(new ZodValidationPipe(markChatSessionReadSchema))
    body: z.infer<typeof markChatSessionReadSchema>,
  ) {
    return this.chat.markSessionRead(
      p.userId,
      sessionId,
      body.lastReadMessageId,
    );
  }

  @Get('chat-sessions/:sessionId/messages')
  listMessages(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('sessionId') sessionId: string,
    @Query(new ZodValidationPipe(chatMessagesQuerySchema))
    query: z.infer<typeof chatMessagesQuerySchema>,
  ) {
    return this.chat.listMessages(
      p.userId,
      sessionId,
      query.cursor,
      query.limit,
    );
  }

  @Post('chat-sessions/:sessionId/messages')
  createMessage(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('sessionId') sessionId: string,
    @Body(new ZodValidationPipe(createChatMessageSchema))
    body: z.infer<typeof createChatMessageSchema>,
  ) {
    return this.chat.createMessage(p.userId, sessionId, body.content);
  }

  @Post('chat-sessions/:sessionId/command-drafts')
  createCommandDraft(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('sessionId') sessionId: string,
    @Body(new ZodValidationPipe(createCommandDraftSchema))
    body: z.infer<typeof createCommandDraftSchema>,
  ) {
    return this.chat.createCommandDraft(p.userId, sessionId, body);
  }

  @Post('command-drafts/:draftId/confirm')
  confirmCommandDraft(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('draftId') draftId: string,
    @Headers('idempotency-key') rawKey: string | undefined,
    @Body(new ZodValidationPipe(confirmCommandDraftSchema))
    _body: z.infer<typeof confirmCommandDraftSchema>,
  ) {
    const key = requireIdempotencyKey(rawKey);
    return this.chat.confirmCommandDraft(p.userId, draftId, key);
  }

  @Post('command-drafts/:draftId/reject')
  rejectCommandDraft(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('draftId') draftId: string,
    @Body(new ZodValidationPipe(rejectCommandDraftSchema))
    _body: z.infer<typeof rejectCommandDraftSchema>,
  ) {
    return this.chat.rejectCommandDraft(p.userId, draftId);
  }

  @Get('rooms/:roomId/chat')
  listLegacyRoomMessages(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('roomId') roomId: string,
  ) {
    return this.chat.listLegacyRoomMessages(p.userId, roomId);
  }

  @Post('rooms/:roomId/chat')
  createLegacyRoomMessage(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('roomId') roomId: string,
    @Headers('idempotency-key') _unusedLegacyKey: string | undefined,
    @Body(new ZodValidationPipe(createChatMessageSchema))
    body: z.infer<typeof createChatMessageSchema>,
  ) {
    return this.chat.createLegacyRoomMessage(p.userId, roomId, body.content);
  }
}
