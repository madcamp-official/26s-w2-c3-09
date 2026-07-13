import {
  ConflictException,
  Inject,
  Injectable,
  NotFoundException,
} from '@nestjs/common';
import { createRuleSchema, updateRuleSchema } from '@mousekeeper/contracts';
import { rooms, rules, type Database } from '@mousekeeper/database';
import { and, asc, eq } from 'drizzle-orm';
import { z } from 'zod';
import { DATABASE } from '../database/database.module';
import { SyncService } from '../sync/sync.service';

@Injectable()
export class RulesService {
  constructor(
    @Inject(DATABASE) private readonly db: Database,
    private readonly sync: SyncService,
  ) {}

  private async ownedRoom(userId: string, roomId: string) {
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
    if (!room) {
      throw new NotFoundException({
        code: 'NOT_FOUND',
        message: 'Room not found',
      });
    }
    return room;
  }

  async list(userId: string, roomId: string) {
    await this.ownedRoom(userId, roomId);
    return this.db
      .select()
      .from(rules)
      .where(eq(rules.roomId, roomId))
      .orderBy(asc(rules.priority), asc(rules.createdAt));
  }

  async create(
    userId: string,
    roomId: string,
    body: z.infer<typeof createRuleSchema>,
  ) {
    const room = await this.ownedRoom(userId, roomId);
    return this.db.transaction(async (tx) => {
      const created = (
        await tx
          .insert(rules)
          .values({ roomId, ...body })
          .returning()
      )[0];
      await this.sync.append(tx, {
        userId,
        deviceId: room.desktopDeviceId,
        roomId,
        eventType: 'rule.created',
        aggregateType: 'rule',
        aggregateId: created.id,
        payload: { ruleId: created.id, version: created.version },
      });
      return created;
    });
  }

  async update(
    userId: string,
    ruleId: string,
    body: z.infer<typeof updateRuleSchema>,
  ) {
    const current = (
      await this.db
        .select({ rule: rules, room: rooms })
        .from(rules)
        .innerJoin(rooms, eq(rules.roomId, rooms.id))
        .where(
          and(
            eq(rules.id, ruleId),
            eq(rooms.userId, userId),
            eq(rooms.status, 'ACTIVE'),
          ),
        )
        .limit(1)
    )[0];
    if (!current) {
      throw new NotFoundException({
        code: 'NOT_FOUND',
        message: 'Rule not found',
      });
    }
    const { version, ...changes } = body;
    return this.db.transaction(async (tx) => {
      const updated = (
        await tx
          .update(rules)
          .set({ ...changes, version: version + 1, updatedAt: new Date() })
          .where(and(eq(rules.id, ruleId), eq(rules.version, version)))
          .returning()
      )[0];
      if (!updated) {
        throw new ConflictException({
          code: 'VERSION_CONFLICT',
          message: 'Rule was changed by another client',
          currentVersion: current.rule.version,
        });
      }
      await this.sync.append(tx, {
        userId,
        deviceId: current.room.desktopDeviceId,
        roomId: current.room.id,
        eventType: 'rule.updated',
        aggregateType: 'rule',
        aggregateId: updated.id,
        payload: { ruleId: updated.id, version: updated.version },
      });
      return updated;
    });
  }
}
