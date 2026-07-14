import { randomUUID } from 'node:crypto';
import {
  commands,
  createDatabase,
  devices,
  rooms,
  syncEvents,
  users,
} from '@mousekeeper/database';
import { eq } from 'drizzle-orm';
import { eventEnvelopeSchema } from '@mousekeeper/contracts';
import { CommandsService } from './commands.service';
import { SyncService } from '../sync/sync.service';

const databaseUrl = process.env.DATABASE_URL;
const describeDatabase =
  databaseUrl && process.env.MOUSEKEEPER_RUN_DB_TESTS === 'true'
    ? describe
    : describe.skip;

describeDatabase('CommandsService PostgreSQL integration', () => {
  let connection: ReturnType<typeof createDatabase>;
  let service: CommandsService;
  let sync: SyncService;
  let userId: string;
  let deviceId: string;
  let roomId: string;

  beforeAll(async () => {
    connection = createDatabase(databaseUrl!);
    sync = new SyncService();
    service = new CommandsService(connection.db, sync);
    const user = (
      await connection.db
        .insert(users)
        .values({
          authProviderUid: `test-${randomUUID()}`,
          displayName: 'Integration User',
        })
        .returning()
    )[0]!;
    const device = (
      await connection.db
        .insert(devices)
        .values({
          userId: user.id,
          platform: 'WINDOWS',
          deviceName: 'Integration PC',
        })
        .returning()
    )[0]!;
    const room = (
      await connection.db
        .insert(rooms)
        .values({
          userId: user.id,
          desktopDeviceId: device.id,
          name: 'Integration Room',
          rootAlias: 'Downloads',
        })
        .returning()
    )[0]!;
    userId = user.id;
    deviceId = device.id;
    roomId = room.id;
  });

  it('persists once per idempotency key and enforces delivery order', async () => {
    const key = randomUUID();
    const first = await service.create(userId, roomId, key, {
      intent: 'ANALYZE',
      payload: {},
      metadata: { idempotencyKey: key, requiresApproval: false },
    });
    const repeated = await service.create(userId, roomId, key, {
      intent: 'ANALYZE',
      payload: {},
      metadata: { requiresApproval: false, idempotencyKey: key },
    });
    expect(repeated.id).toBe(first.id);
    expect(first.metadata).toEqual({
      idempotencyKey: key,
      requiresApproval: false,
    });
    expect(first).not.toHaveProperty('idempotencyKey');
    expect(first).not.toHaveProperty('createdByUserId');
    await expect(
      service.create(userId, roomId, key, {
        intent: 'ANALYZE',
        payload: {},
        metadata: { idempotencyKey: key, requiresApproval: true },
      }),
    ).rejects.toMatchObject({ response: { code: 'IDEMPOTENCY_CONFLICT' } });
    await expect(
      service.create(userId, roomId, key, { intent: 'SCAN', payload: {} }),
    ).rejects.toMatchObject({ response: { code: 'IDEMPOTENCY_CONFLICT' } });
    expect(
      (await service.pending(userId, deviceId)).map((item) => item.id),
    ).toContain(first.id);
    expect(
      (
        await service.update(userId, deviceId, first.id, {
          status: 'DELIVERED',
        })
      ).status,
    ).toBe('DELIVERED');
    expect(
      (
        await service.update(userId, deviceId, first.id, {
          status: 'ANALYZING',
        })
      ).status,
    ).toBe('ANALYZING');
    const replay = await sync.replay(connection.db, userId, 0, 100);
    expect(replay.length).toBeGreaterThanOrEqual(3);
    expect(
      replay.every((event) => eventEnvelopeSchema.safeParse(event).success),
    ).toBe(true);
    expect(replay.map((event) => event.sequence)).toEqual(
      [...replay.map((event) => event.sequence)].sort((a, b) => a - b),
    );
  });

  afterAll(async () => {
    await connection.db.delete(syncEvents).where(eq(syncEvents.userId, userId));
    await connection.db.delete(commands).where(eq(commands.roomId, roomId));
    await connection.db.delete(rooms).where(eq(rooms.id, roomId));
    await connection.db.delete(devices).where(eq(devices.id, deviceId));
    await connection.db.delete(users).where(eq(users.id, userId));
    await connection.close();
  });
});
