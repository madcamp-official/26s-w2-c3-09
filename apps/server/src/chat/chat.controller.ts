import {
  Body,
  Controller,
  Get,
  Inject,
  NotFoundException,
  Param,
  Post,
  UseGuards,
} from '@nestjs/common';
import { createChatMessageSchema } from '@mousekeeper/contracts';
import { chatMessages, rooms, type Database } from '@mousekeeper/database';
import { and, asc, eq } from 'drizzle-orm';
import { z } from 'zod';
import { CurrentPrincipal } from '../auth/auth-principal';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import { ZodValidationPipe } from '../common/zod-validation.pipe';
import { DATABASE } from '../database/database.module';
import { SyncService } from '../sync/sync.service';
@Controller('v1/rooms/:roomId/chat')
@UseGuards(FirebaseAuthGuard)
export class ChatController {
  constructor(
    @Inject(DATABASE) private readonly db: Database,
    private readonly sync: SyncService,
  ) {}
  private async owned(userId: string, roomId: string) {
    const room = (
      await this.db
        .select()
        .from(rooms)
        .where(
          and(
            eq(rooms.id, roomId),
            eq(rooms.userId, userId),
            eq(rooms.status, 'ACTIVE'),
          ),
        )
        .limit(1)
    )[0];
    if (!room) throw new NotFoundException({ code: 'NOT_FOUND' });
  }
  @Get() async list(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('roomId') roomId: string,
  ) {
    await this.owned(p.userId, roomId);
    return this.db
      .select()
      .from(chatMessages)
      .where(eq(chatMessages.roomId, roomId))
      .orderBy(asc(chatMessages.createdAt))
      .limit(200);
  }
  @Post() async create(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('roomId') roomId: string,
    @Body(new ZodValidationPipe(createChatMessageSchema))
    body: z.infer<typeof createChatMessageSchema>,
  ) {
    await this.owned(p.userId, roomId);
    const message = await this.db.transaction(async (tx) => {
      const created = (
        await tx
          .insert(chatMessages)
          .values({ roomId, senderType: 'USER', content: body.content })
          .returning()
      )[0]!;
      await this.sync.append(tx, {
        userId: p.userId,
        deviceId: null,
        roomId,
        eventType: 'chat.message.created',
        aggregateType: 'chat_message',
        aggregateId: created.id,
        payload: { messageId: created.id, senderType: created.senderType },
      });
      return created;
    });
    return { message, assistant: null, aiStatus: 'UNCONFIGURED' as const };
  }
}
