import { randomUUID } from 'node:crypto';
import {
  createDatabase,
  devices,
  rooms,
  ruleDrafts,
  rules,
  syncEvents,
  users,
} from '@mousekeeper/database';
import { eq } from 'drizzle-orm';
import type {
  AiProvider,
  AiProviderResult,
  ChatContext,
  RuleDraftResult,
  RuleTranslationContext,
} from '../ai/ai.provider';
import { UnconfiguredAiProvider } from '../ai/unconfigured-ai.provider';
import { SyncService } from '../sync/sync.service';
import { RulesService } from './rules.service';

const databaseUrl = process.env.DATABASE_URL;
const describeDatabase =
  databaseUrl && process.env.MOUSEKEEPER_RUN_DB_TESTS === 'true'
    ? describe
    : describe.skip;

class ScriptedRuleAiProvider implements AiProvider {
  async classifyAndRespond(_input: ChatContext): Promise<AiProviderResult> {
    return { status: 'UNCONFIGURED', code: 'AI_PROVIDER_UNCONFIGURED' };
  }

  async translateRule(
    _input: RuleTranslationContext,
  ): Promise<RuleDraftResult> {
    return {
      status: 'READY',
      kind: 'RULE_DRAFT',
      draft: {
        name: 'Old PDFs',
        definition: {
          match: 'ALL',
          conditions: [
            { field: 'modifiedAgeDays', operator: 'GTE', value: 30 },
            { field: 'extension', operator: 'IN', value: ['.pdf'] },
          ],
          action: { type: 'MOVE', destinationTemplate: 'Archive/PDF' },
        },
        explanation: 'Move old PDFs after explicit approval.',
        ambiguities: [],
      },
    };
  }
}

describeDatabase('RulesService rule draft integration', () => {
  let connection: ReturnType<typeof createDatabase>;
  let userId: string;
  let deviceId: string;
  let roomId: string;

  beforeEach(async () => {
    connection = createDatabase(databaseUrl!);
    const user = (
      await connection.db
        .insert(users)
        .values({
          authProviderUid: `rule-draft-${randomUUID()}`,
          displayName: 'Rule User',
        })
        .returning()
    )[0]!;
    const device = (
      await connection.db
        .insert(devices)
        .values({
          userId: user.id,
          platform: 'WINDOWS',
          deviceName: 'Rule PC',
        })
        .returning()
    )[0]!;
    const room = (
      await connection.db
        .insert(rooms)
        .values({
          userId: user.id,
          desktopDeviceId: device.id,
          name: 'Rule Room',
          rootAlias: 'Downloads',
        })
        .returning()
    )[0]!;
    userId = user.id;
    deviceId = device.id;
    roomId = room.id;
  });

  afterEach(async () => {
    if (userId) {
      await connection.db
        .delete(syncEvents)
        .where(eq(syncEvents.userId, userId));
      await connection.db
        .delete(ruleDrafts)
        .where(eq(ruleDrafts.createdByUserId, userId));
    }
    if (roomId) {
      await connection.db.delete(rules).where(eq(rules.roomId, roomId));
      await connection.db.delete(rooms).where(eq(rooms.id, roomId));
    }
    if (deviceId) {
      await connection.db.delete(devices).where(eq(devices.id, deviceId));
    }
    if (userId) {
      await connection.db.delete(users).where(eq(users.id, userId));
    }
    await connection.close();
  });

  it('keeps unconfigured AI as an explicit state without storing a fake draft', async () => {
    const service = new RulesService(
      connection.db,
      new SyncService(),
      new UnconfiguredAiProvider(),
    );

    const result = await service.createDraft(userId, roomId, {
      instruction: 'Move old PDFs',
    });

    expect(result).toEqual({
      status: 'UNCONFIGURED',
      code: 'AI_PROVIDER_UNCONFIGURED',
    });
    expect(
      await connection.db
        .select()
        .from(ruleDrafts)
        .where(eq(ruleDrafts.roomId, roomId)),
    ).toEqual([]);
  });

  it('persists a READY draft and materializes it only after idempotent confirmation', async () => {
    const service = new RulesService(
      connection.db,
      new SyncService(),
      new ScriptedRuleAiProvider(),
    );

    const result = await service.createDraft(userId, roomId, {
      instruction: 'Move old PDFs',
    });
    expect(result.status).toBe('READY');
    if (result.status !== 'READY') throw new Error('expected ready draft');
    expect(result.draft.status).toBe('DRAFT');
    expect(await service.list(userId, roomId)).toEqual([]);

    const confirmed = await service.confirmDraft(
      userId,
      result.draft.id,
      'rule-confirm-key',
    );
    expect(confirmed.draft.status).toBe('MATERIALIZED');
    expect(confirmed.draft.ruleId).toBe(confirmed.rule.id);
    expect(confirmed.rule).toMatchObject({
      roomId,
      name: 'Old PDFs',
      priority: 100,
      enabled: true,
      version: 1,
    });

    await expect(
      service.confirmDraft(userId, result.draft.id, 'another-confirm-key'),
    ).rejects.toMatchObject({
      response: { code: 'IDEMPOTENCY_CONFLICT' },
    });
    await expect(
      service.confirmDraft(userId, result.draft.id, 'rule-confirm-key'),
    ).resolves.toMatchObject({
      rule: { id: confirmed.rule.id },
      draft: { id: result.draft.id, status: 'MATERIALIZED' },
    });
  });

  it('fails closed for rule draft preview until desktop dry-run transport is configured', async () => {
    const service = new RulesService(
      connection.db,
      new SyncService(),
      new ScriptedRuleAiProvider(),
    );

    const result = await service.createDraft(userId, roomId, {
      instruction: 'Move old PDFs',
    });
    if (result.status !== 'READY') throw new Error('expected ready draft');

    await expect(
      service.previewDraft(userId, result.draft.id),
    ).rejects.toMatchObject({
      response: {
        code: 'RULE_DRAFT_PREVIEW_UNCONFIGURED',
        draft: { id: result.draft.id, status: 'DRAFT' },
      },
    });
    expect(await service.list(userId, roomId)).toEqual([]);
  });

  it('rejects a draft without creating a rule', async () => {
    const service = new RulesService(
      connection.db,
      new SyncService(),
      new ScriptedRuleAiProvider(),
    );
    const result = await service.createDraft(userId, roomId, {
      instruction: 'Move old PDFs',
    });
    if (result.status !== 'READY') throw new Error('expected ready draft');

    await expect(
      service.rejectDraft(userId, result.draft.id),
    ).resolves.toMatchObject({
      draft: { id: result.draft.id, status: 'REJECTED', ruleId: null },
    });
    await expect(
      service.confirmDraft(userId, result.draft.id, 'reject-confirm-key'),
    ).rejects.toMatchObject({
      response: { code: 'INVALID_STATE_TRANSITION' },
    });
    expect(await service.list(userId, roomId)).toEqual([]);
  });
});
