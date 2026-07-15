import {
  Body,
  Controller,
  Delete,
  ForbiddenException,
  Get,
  Headers,
  Inject,
  NotFoundException,
  Param,
  Patch,
  Post,
  UseGuards,
} from '@nestjs/common';
import {
  createRoomSchema,
  roomCreatedEventPayloadSchema,
  updateRoomAiDocumentConsentSchema,
} from '@mousekeeper/contracts';
import { devices, rooms, type Database } from '@mousekeeper/database';
import { and, eq } from 'drizzle-orm';
import { z } from 'zod';
import { AgentOnly } from '../auth/agent-only.decorator';
import { requireAgentDevice } from '../auth/agent-device';
import { CurrentPrincipal } from '../auth/auth-principal';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import { ZodValidationPipe } from '../common/zod-validation.pipe';
import {
  ConnectionLifecycleService,
  mobileConnectionActor,
} from '../connections/connection-lifecycle.service';
import { requireIdempotencyKey } from '../connections/idempotency-key';
import { DATABASE } from '../database/database.module';
import { RealtimeDispatcher } from '../realtime/realtime-dispatcher.service';
import { SyncService } from '../sync/sync.service';

@Controller('v1/rooms')
@UseGuards(FirebaseAuthGuard)
export class RoomsController {
  constructor(
    @Inject(DATABASE) private readonly db: Database,
    private readonly lifecycle: ConnectionLifecycleService,
    private readonly sync: SyncService,
    private readonly realtime: RealtimeDispatcher,
  ) {}

  @Get()
  async list(@CurrentPrincipal() principal: AuthPrincipal) {
    const result = await this.db
      .select()
      .from(rooms)
      .where(
        and(eq(rooms.userId, principal.userId), eq(rooms.status, 'ACTIVE')),
      );
    return result.map((room) => this.publicRoom(room));
  }

  @Post()
  @AgentOnly()
  async create(
    @CurrentPrincipal() principal: AuthPrincipal,
    @Body(new ZodValidationPipe(createRoomSchema))
    body: z.infer<typeof createRoomSchema>,
  ) {
    requireAgentDevice(principal, body.desktopDeviceId);
    const outcome = await this.db.transaction(async (tx) => {
      const device = (
        await tx
          .select()
          .from(devices)
          .where(
            and(
              eq(devices.id, body.desktopDeviceId),
              eq(devices.userId, principal.userId),
              eq(devices.status, 'ACTIVE'),
            ),
          )
          .for('share')
          .limit(1)
      )[0];
      if (!device) {
        throw new NotFoundException({
          code: 'NOT_FOUND',
          message: 'Desktop device not found',
        });
      }
      const created = (
        await tx
          .insert(rooms)
          .values({ userId: principal.userId, ...body })
          .returning()
      )[0];
      const room = this.publicRoom(created);
      const event = await this.sync.append(tx, {
        userId: principal.userId,
        deviceId: created.desktopDeviceId,
        roomId: created.id,
        eventType: 'room.created',
        aggregateType: 'room',
        aggregateId: created.id,
        payload: roomCreatedEventPayloadSchema.parse({
          roomId: created.id,
          status: 'ACTIVE',
          room,
        }),
      });
      return { room, eventId: event.id };
    });
    try {
      await this.realtime.publishNow([outcome.eventId]);
    } catch {
      // The durable sync event remains available to the dispatcher and replay.
      console.error('ROOM_CREATED_IMMEDIATE_PUBLISH_FAILED');
    }
    return outcome.room;
  }

  @Delete(':id')
  remove(
    @CurrentPrincipal() principal: AuthPrincipal,
    @Param('id') id: string,
    @Headers('idempotency-key') rawKey: string | undefined,
  ) {
    if (principal.authType !== 'FIREBASE') {
      throw new ForbiddenException({ code: 'FORBIDDEN' });
    }
    return this.lifecycle.removeRoom(
      mobileConnectionActor(principal.userId),
      id,
      requireIdempotencyKey(rawKey),
    );
  }

  @Patch(':id/ai-document-consent')
  async updateAiDocumentConsent(
    @CurrentPrincipal() principal: AuthPrincipal,
    @Param('id') id: string,
    @Body(new ZodValidationPipe(updateRoomAiDocumentConsentSchema))
    body: z.infer<typeof updateRoomAiDocumentConsentSchema>,
  ) {
    if (principal.authType !== 'FIREBASE') {
      throw new ForbiddenException({ code: 'FORBIDDEN' });
    }
    return this.db.transaction(async (tx) => {
      const updated = (
        await tx
          .update(rooms)
          .set({ aiDocumentAnalysisConsent: body.enabled })
          .where(
            and(
              eq(rooms.id, id),
              eq(rooms.userId, principal.userId),
              eq(rooms.status, 'ACTIVE'),
            ),
          )
          .returning()
      )[0];
      if (!updated) throw new NotFoundException({ code: 'NOT_FOUND' });
      await this.sync.append(tx, {
        userId: principal.userId,
        deviceId: updated.desktopDeviceId,
        roomId: updated.id,
        eventType: 'room.ai_document_consent.updated',
        aggregateType: 'room',
        aggregateId: updated.id,
        payload: { roomId: updated.id, enabled: body.enabled },
      });
      return this.publicRoom(updated);
    });
  }

  private publicRoom(room: typeof rooms.$inferSelect) {
    const { userId: _, ...safe } = room;
    return { ...safe, createdAt: safe.createdAt.toISOString() };
  }
}
