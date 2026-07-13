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
  Post,
  UseGuards,
} from '@nestjs/common';
import { createRoomSchema } from '@mousekeeper/contracts';
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

@Controller('v1/rooms')
@UseGuards(FirebaseAuthGuard)
export class RoomsController {
  constructor(
    @Inject(DATABASE) private readonly db: Database,
    private readonly lifecycle: ConnectionLifecycleService,
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
    return this.db.transaction(async (tx) => {
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
      return this.publicRoom(created);
    });
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

  private publicRoom(room: typeof rooms.$inferSelect) {
    const { userId: _, ...safe } = room;
    return safe;
  }
}
