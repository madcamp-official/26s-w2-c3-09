import {
  Body,
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
  createChatMessageSchema,
  createChatSessionSchema,
  updateChatSessionSchema,
} from '@mousekeeper/contracts';
import { z } from 'zod';
import { CurrentPrincipal } from '../auth/auth-principal';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import { ZodValidationPipe } from '../common/zod-validation.pipe';
import { ChatService } from './chat.service';

@Controller('v1')
@UseGuards(FirebaseAuthGuard)
export class ChatController {
  constructor(private readonly chat: ChatService) {}

  @Get('rooms/:roomId/chat-sessions')
  listSessions(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('roomId') roomId: string,
  ) {
    return this.chat.listSessions(p.userId, roomId);
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
