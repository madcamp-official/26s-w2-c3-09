import {
  ConflictException,
  Inject,
  Injectable,
  NotFoundException,
} from '@nestjs/common';
import {
  createRuleDraftRequestSchema,
  createRuleSchema,
  updateRuleSchema,
} from '@mousekeeper/contracts';
import { rooms, ruleDrafts, rules, type Database } from '@mousekeeper/database';
import { and, asc, desc, eq } from 'drizzle-orm';
import { z } from 'zod';
import { AI_PROVIDER, type AiProvider } from '../ai/ai.provider';
import { DATABASE } from '../database/database.module';
import { SyncService } from '../sync/sync.service';

const DEFAULT_RULE_DRAFT_TTL_MS = 10 * 60 * 1000;
const RULE_PRIORITY_STEP = 100;
type Transaction = Parameters<Parameters<Database['transaction']>[0]>[0];
type DbExecutor = Database | Transaction;

@Injectable()
export class RulesService {
  constructor(
    @Inject(DATABASE) private readonly db: Database,
    private readonly sync: SyncService,
    @Inject(AI_PROVIDER) private readonly ai: AiProvider,
  ) {}

  private async ownedRoom(userId: string, roomId: string) {
    return this.ownedRoomIn(this.db, userId, roomId);
  }

  private async ownedRoomIn(db: DbExecutor, userId: string, roomId: string) {
    const room = (
      await db
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

  async createDraft(
    userId: string,
    roomId: string,
    body: z.infer<typeof createRuleDraftRequestSchema>,
  ) {
    await this.ownedRoom(userId, roomId);
    const translated = await this.ai.translateRule({
      userId,
      roomId,
      instruction: body.instruction,
    });
    if (translated.status !== 'READY') return translated;

    return this.db.transaction(async (tx) => {
      await this.ownedRoomIn(tx, userId, roomId);
      const draft = (
        await tx
          .insert(ruleDrafts)
          .values({
            roomId,
            createdByUserId: userId,
            name: translated.draft.name,
            definition: translated.draft.definition,
            explanation: translated.draft.explanation,
            ambiguities: translated.draft.ambiguities,
            expiresAt: new Date(Date.now() + DEFAULT_RULE_DRAFT_TTL_MS),
          })
          .returning()
      )[0]!;
      await this.sync.append(tx, {
        userId,
        deviceId: null,
        roomId,
        eventType: 'rule.draft.created',
        aggregateType: 'rule_draft',
        aggregateId: draft.id,
        payload: { ruleDraftId: draft.id, status: draft.status },
      });
      return {
        status: 'READY' as const,
        kind: 'RULE_DRAFT' as const,
        draft: this.publicRuleDraft(draft),
      };
    });
  }

  async confirmDraft(userId: string, draftId: string, key: string) {
    return this.db.transaction(async (tx) => {
      const owned = await this.requireOwnedDraftIn(tx, userId, draftId);
      const draft = owned.draft;
      if (draft.status === 'MATERIALIZED' && draft.ruleId) {
        if (draft.confirmIdempotencyKey !== key) {
          throw new ConflictException({ code: 'IDEMPOTENCY_CONFLICT' });
        }
        const rule = await this.requireRuleIn(tx, draft.ruleId);
        return {
          draft: this.publicRuleDraft(draft),
          rule: this.publicRule(rule),
        };
      }
      if (draft.status === 'REJECTED') {
        throw new ConflictException({
          code: 'INVALID_STATE_TRANSITION',
          from: draft.status,
          to: 'MATERIALIZED',
        });
      }
      if (draft.status === 'EXPIRED' || draft.expiresAt <= new Date()) {
        const expired =
          draft.status === 'EXPIRED'
            ? draft
            : (
                await tx
                  .update(ruleDrafts)
                  .set({ status: 'EXPIRED' })
                  .where(eq(ruleDrafts.id, draft.id))
                  .returning()
              )[0]!;
        throw new ConflictException({
          code: 'DRAFT_EXPIRED',
          draft: this.publicRuleDraft(expired),
        });
      }
      if (draft.status !== 'DRAFT') {
        throw new ConflictException({
          code: 'INVALID_STATE_TRANSITION',
          from: draft.status,
          to: 'MATERIALIZED',
        });
      }
      const keyOwner = (
        await tx
          .select()
          .from(ruleDrafts)
          .where(
            and(
              eq(ruleDrafts.createdByUserId, userId),
              eq(ruleDrafts.confirmIdempotencyKey, key),
            ),
          )
          .limit(1)
      )[0];
      if (keyOwner && keyOwner.id !== draft.id) {
        throw new ConflictException({ code: 'IDEMPOTENCY_CONFLICT' });
      }
      const priority = await this.nextRulePriorityIn(tx, owned.room.id);
      const rule = (
        await tx
          .insert(rules)
          .values({
            roomId: owned.room.id,
            name: draft.name,
            definition: draft.definition,
            priority,
            enabled: true,
          })
          .returning()
      )[0]!;
      const updated = (
        await tx
          .update(ruleDrafts)
          .set({
            status: 'MATERIALIZED',
            ruleId: rule.id,
            confirmIdempotencyKey: key,
          })
          .where(eq(ruleDrafts.id, draft.id))
          .returning()
      )[0]!;
      await this.sync.append(tx, {
        userId,
        deviceId: owned.room.desktopDeviceId,
        roomId: owned.room.id,
        eventType: 'rule.created',
        aggregateType: 'rule',
        aggregateId: rule.id,
        payload: {
          ruleId: rule.id,
          version: rule.version,
          ruleDraftId: draft.id,
        },
      });
      await this.sync.append(tx, {
        userId,
        deviceId: null,
        roomId: owned.room.id,
        eventType: 'rule.draft.updated',
        aggregateType: 'rule_draft',
        aggregateId: draft.id,
        payload: {
          ruleDraftId: draft.id,
          status: updated.status,
          ruleId: rule.id,
        },
      });
      return {
        draft: this.publicRuleDraft(updated),
        rule: this.publicRule(rule),
      };
    });
  }

  async rejectDraft(userId: string, draftId: string) {
    return this.db.transaction(async (tx) => {
      const owned = await this.requireOwnedDraftIn(tx, userId, draftId);
      const draft = owned.draft;
      if (draft.status === 'REJECTED') {
        return { draft: this.publicRuleDraft(draft) };
      }
      if (draft.status === 'MATERIALIZED') {
        throw new ConflictException({
          code: 'INVALID_STATE_TRANSITION',
          from: draft.status,
          to: 'REJECTED',
        });
      }
      const nextStatus = draft.expiresAt <= new Date() ? 'EXPIRED' : 'REJECTED';
      const updated = (
        await tx
          .update(ruleDrafts)
          .set({ status: nextStatus })
          .where(eq(ruleDrafts.id, draft.id))
          .returning()
      )[0]!;
      await this.sync.append(tx, {
        userId,
        deviceId: null,
        roomId: owned.room.id,
        eventType: 'rule.draft.updated',
        aggregateType: 'rule_draft',
        aggregateId: draft.id,
        payload: {
          ruleDraftId: draft.id,
          status: updated.status,
          ruleId: updated.ruleId,
        },
      });
      return { draft: this.publicRuleDraft(updated) };
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

  private async requireOwnedDraftIn(
    tx: Transaction,
    userId: string,
    draftId: string,
  ) {
    const row = (
      await tx
        .select({ draft: ruleDrafts, room: rooms })
        .from(ruleDrafts)
        .innerJoin(
          rooms,
          and(
            eq(rooms.id, ruleDrafts.roomId),
            eq(rooms.userId, userId),
            eq(rooms.status, 'ACTIVE'),
          ),
        )
        .where(
          and(
            eq(ruleDrafts.id, draftId),
            eq(ruleDrafts.createdByUserId, userId),
          ),
        )
        .for('update')
        .limit(1)
    )[0];
    if (!row) throw new NotFoundException({ code: 'NOT_FOUND' });
    return row;
  }

  private async requireRuleIn(tx: Transaction, ruleId: string) {
    const rule = (
      await tx.select().from(rules).where(eq(rules.id, ruleId)).limit(1)
    )[0];
    if (!rule) throw new ConflictException({ code: 'CONFLICT' });
    return rule;
  }

  private async nextRulePriorityIn(tx: Transaction, roomId: string) {
    const last = (
      await tx
        .select({ priority: rules.priority })
        .from(rules)
        .where(eq(rules.roomId, roomId))
        .orderBy(desc(rules.priority), desc(rules.createdAt))
        .limit(1)
    )[0];
    return Math.min((last?.priority ?? 0) + RULE_PRIORITY_STEP, 10000);
  }

  private publicRuleDraft(draft: typeof ruleDrafts.$inferSelect) {
    return {
      id: draft.id,
      roomId: draft.roomId,
      name: draft.name,
      definition: draft.definition,
      explanation: draft.explanation,
      ambiguities: Array.isArray(draft.ambiguities) ? draft.ambiguities : [],
      status: draft.status,
      expiresAt: draft.expiresAt,
      ruleId: draft.ruleId,
    };
  }

  private publicRule(rule: typeof rules.$inferSelect) {
    return {
      id: rule.id,
      roomId: rule.roomId,
      name: rule.name,
      definition: rule.definition,
      priority: rule.priority,
      enabled: rule.enabled,
      version: rule.version,
      createdAt: rule.createdAt,
      updatedAt: rule.updatedAt,
    };
  }
}
