import {
  ConflictException,
  ForbiddenException,
  Inject,
  Injectable,
  NotFoundException,
} from '@nestjs/common';
import {
  createCommandSchema,
  updateCommandStatusSchema,
} from '@mousekeeper/contracts';
import { commands, devices, rooms, type Database } from '@mousekeeper/database';
import { and, asc, eq, inArray } from 'drizzle-orm';
import { z } from 'zod';
import { DATABASE } from '../database/database.module';
import { SyncService } from '../sync/sync.service';
import { canTransition } from './command-state';
import { canonicalJson } from '../common/canonical-json';

@Injectable()
export class CommandsService {
  constructor(
    @Inject(DATABASE) private readonly db: Database,
    private readonly sync: SyncService,
  ) {}

  async create(
    userId: string,
    roomId: string,
    key: string,
    body: z.infer<typeof createCommandSchema>,
  ) {
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
    if (!room)
      throw new NotFoundException({
        code: 'NOT_FOUND',
        message: 'Room not found',
      });
    return this.db.transaction(async (tx) => {
      const created = (
        await tx
          .insert(commands)
          .values({
            roomId,
            targetDeviceId: room.desktopDeviceId,
            createdByUserId: userId,
            intent: body.intent,
            payload: body.payload,
            idempotencyKey: key,
          })
          .onConflictDoNothing()
          .returning()
      )[0];
      if (!created) {
        const existing = (
          await tx
            .select()
            .from(commands)
            .where(
              and(
                eq(commands.createdByUserId, userId),
                eq(commands.idempotencyKey, key),
              ),
            )
            .limit(1)
        )[0];
        if (!existing) throw new ConflictException({ code: 'CONFLICT' });
        if (
          existing.roomId !== roomId ||
          existing.intent !== body.intent ||
          canonicalJson(existing.payload) !== canonicalJson(body.payload)
        )
          throw new ConflictException({ code: 'IDEMPOTENCY_CONFLICT' });
        return this.publicCommand(existing);
      }
      await this.sync.append(tx, {
        userId,
        deviceId: room.desktopDeviceId,
        roomId,
        eventType: 'command.available',
        aggregateType: 'command',
        aggregateId: created.id,
        payload: { commandId: created.id },
      });
      return this.publicCommand(created);
    });
  }

  async pending(userId: string, deviceId: string) {
    const device = (
      await this.db
        .select()
        .from(devices)
        .where(
          and(
            eq(devices.id, deviceId),
            eq(devices.userId, userId),
            eq(devices.status, 'ACTIVE'),
          ),
        )
        .limit(1)
    )[0];
    if (!device) throw new ForbiddenException({ code: 'FORBIDDEN' });
    const pending = await this.db
      .select()
      .from(commands)
      .where(
        and(
          eq(commands.targetDeviceId, deviceId),
          inArray(commands.status, ['QUEUED', 'DELIVERED', 'ANALYZING']),
        ),
      )
      .orderBy(asc(commands.createdAt));
    return pending.map((command) => this.publicCommand(command));
  }

  async listForRoom(userId: string, roomId: string) {
    const room = (
      await this.db
        .select()
        .from(rooms)
        .where(and(eq(rooms.id, roomId), eq(rooms.userId, userId)))
        .limit(1)
    )[0];
    if (!room) throw new NotFoundException({ code: 'NOT_FOUND' });
    const result = await this.db
      .select()
      .from(commands)
      .where(eq(commands.roomId, roomId))
      .orderBy(asc(commands.createdAt));
    return result.map((command) => this.publicCommand(command));
  }

  async update(
    userId: string,
    deviceId: string,
    commandId: string,
    body: z.infer<typeof updateCommandStatusSchema>,
  ) {
    return this.db.transaction(async (tx) => {
      const current = (
        await tx
          .select({ command: commands, device: devices })
          .from(commands)
          .innerJoin(devices, eq(commands.targetDeviceId, devices.id))
          .where(
            and(
              eq(commands.id, commandId),
              eq(commands.targetDeviceId, deviceId),
              eq(devices.userId, userId),
            ),
          )
          .limit(1)
      )[0];
      if (!current) throw new NotFoundException({ code: 'NOT_FOUND' });
      if (!canTransition(current.command.status, body.status))
        throw new ConflictException({
          code: 'INVALID_STATE_TRANSITION',
          from: current.command.status,
          to: body.status,
        });
      const updated = (
        await tx
          .update(commands)
          .set({
            status: body.status,
            ...(body.status === 'DELIVERED' ? { deliveredAt: new Date() } : {}),
          })
          .where(eq(commands.id, commandId))
          .returning()
      )[0];
      if (!updated) throw new ConflictException({ code: 'CONFLICT' });
      await this.sync.append(tx, {
        userId,
        deviceId,
        roomId: updated.roomId,
        eventType: 'command.updated',
        aggregateType: 'command',
        aggregateId: updated.id,
        payload: { status: updated.status },
      });
      return this.publicCommand(updated);
    });
  }

  private publicCommand(command: typeof commands.$inferSelect) {
    const { createdByUserId: _, idempotencyKey: __, ...safe } = command;
    return safe;
  }
}
