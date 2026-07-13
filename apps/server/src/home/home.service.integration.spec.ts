import { randomUUID } from 'node:crypto';
import {
  characterProfiles,
  commands,
  createDatabase,
  decisions,
  devices,
  executions,
  proposals,
  roomSnapshots,
  rooms,
  users,
} from '@mousekeeper/database';
import { eq, inArray } from 'drizzle-orm';
import Redis from 'ioredis';
import { CharacterService } from '../character/character.service';
import { HomeService } from './home.service';

const databaseUrl = process.env.DATABASE_URL;
const redisUrl = process.env.REDIS_URL;
const describeIntegration = databaseUrl && redisUrl ? describe : describe.skip;

describeIntegration('HomeService PostgreSQL and Redis integration', () => {
  let connection: ReturnType<typeof createDatabase>;
  let redis: Redis;

  beforeAll(async () => {
    connection = createDatabase(databaseUrl!);
    redis = new Redis(redisUrl!, {
      lazyConnect: true,
      maxRetriesPerRequest: 2,
    });
    await redis.connect();
  });

  afterAll(async () => {
    await connection.close();
    await redis.quit();
  });

  it('returns one validated aggregate without leaking another user or removed rows', async () => {
    const authUid = `home-summary-${randomUUID()}`;
    const otherAuthUid = `home-summary-other-${randomUUID()}`;
    const insertedUsers = await connection.db
      .insert(users)
      .values([
        { authProviderUid: authUid, displayName: 'Home owner' },
        { authProviderUid: otherAuthUid, displayName: 'Other owner' },
      ])
      .returning();
    const user = insertedUsers.find((row) => row.authProviderUid === authUid)!;
    const otherUser = insertedUsers.find(
      (row) => row.authProviderUid === otherAuthUid,
    )!;
    const insertedDevices = await connection.db
      .insert(devices)
      .values([
        {
          userId: user.id,
          platform: 'WINDOWS',
          deviceName: 'Active desktop',
        },
        {
          userId: user.id,
          platform: 'WINDOWS',
          deviceName: 'Revoked desktop',
          status: 'REVOKED',
        },
        {
          userId: otherUser.id,
          platform: 'WINDOWS',
          deviceName: 'Other desktop',
        },
      ])
      .returning();
    const device = insertedDevices.find(
      (row) => row.deviceName === 'Active desktop',
    )!;
    const revokedDevice = insertedDevices.find(
      (row) => row.deviceName === 'Revoked desktop',
    )!;
    const otherDevice = insertedDevices.find(
      (row) => row.deviceName === 'Other desktop',
    )!;
    const insertedRooms = await connection.db
      .insert(rooms)
      .values([
        {
          userId: user.id,
          desktopDeviceId: device.id,
          name: 'Downloads',
          rootAlias: 'Downloads',
        },
        {
          userId: user.id,
          desktopDeviceId: revokedDevice.id,
          name: 'Removed room',
          rootAlias: 'Removed',
          status: 'REMOVED',
        },
        {
          userId: otherUser.id,
          desktopDeviceId: otherDevice.id,
          name: 'Other room',
          rootAlias: 'Other',
        },
      ])
      .returning();
    const room = insertedRooms.find((row) => row.name === 'Downloads')!;
    const commandRows = await connection.db
      .insert(commands)
      .values([
        {
          roomId: room.id,
          targetDeviceId: device.id,
          createdByUserId: user.id,
          intent: 'ANALYZE',
          payload: {},
          status: 'WAITING_APPROVAL',
          idempotencyKey: `home-open-${randomUUID()}`,
        },
        {
          roomId: room.id,
          targetDeviceId: device.id,
          createdByUserId: user.id,
          intent: 'ANALYZE',
          payload: {},
          status: 'SUCCEEDED',
          idempotencyKey: `home-finished-${randomUUID()}`,
        },
      ])
      .returning();
    const proposalRows = await connection.db
      .insert(proposals)
      .values([
        {
          commandId: commandRows[0].id,
          roomId: room.id,
          status: 'OPEN',
          summary: {},
          idempotencyKey: `home-proposal-open-${randomUUID()}`,
        },
        {
          commandId: commandRows[1].id,
          roomId: room.id,
          status: 'APPROVED',
          summary: {},
          idempotencyKey: `home-proposal-approved-${randomUUID()}`,
        },
      ])
      .returning();
    const decision = (
      await connection.db
        .insert(decisions)
        .values({
          proposalId: proposalRows[1].id,
          userId: user.id,
          decisionType: 'APPROVE',
          approvedItemIds: [],
          idempotencyKey: `home-decision-${randomUUID()}`,
        })
        .returning()
    )[0];
    await connection.db.insert(executions).values({
      proposalId: proposalRows[1].id,
      decisionId: decision.id,
      desktopDeviceId: device.id,
      status: 'SUCCEEDED',
      idempotencyKey: `home-execution-${randomUUID()}`,
    });
    const older = new Date('2026-07-12T00:00:00.000Z');
    const latest = new Date('2026-07-13T00:00:00.000Z');
    await connection.db.insert(roomSnapshots).values([
      {
        roomId: room.id,
        score: 41,
        metrics: {},
        formulaVersion: 'mousekeeper-cleanliness-v1',
        calculatedAt: older,
      },
      {
        roomId: room.id,
        score: 93,
        metrics: {},
        formulaVersion: 'mousekeeper-cleanliness-v1',
        calculatedAt: latest,
      },
    ]);
    await redis.set(`presence:${device.id}`, 'ONLINE_IDLE', 'EX', 60);
    await redis.set(`presence:${otherDevice.id}`, 'ONLINE_EXECUTING', 'EX', 60);

    try {
      const character = new CharacterService(connection.db);
      const summary = await new HomeService(
        connection.db,
        redis,
        character,
      ).summary(user.id);

      expect(summary.devices).toHaveLength(1);
      expect(summary.devices[0]).toMatchObject({
        id: device.id,
        deviceName: 'Active desktop',
        presence: 'ONLINE_IDLE',
      });
      expect(summary.devices[0]).not.toHaveProperty('userId');
      expect(summary.devices[0]).not.toHaveProperty('publicKey');
      expect(summary.rooms).toEqual([
        expect.objectContaining({
          id: room.id,
          pendingProposalCount: 1,
          latestExecutionStatus: 'SUCCEEDED',
          cleanlinessScore: 93,
          cleanlinessFormulaVersion: 'mousekeeper-cleanliness-v1',
          cleanlinessCalculatedAt: latest.toISOString(),
        }),
      ]);
      expect(summary.character).toMatchObject({
        affinityTotal: 0,
        riveAssetStatus: 'UNCONFIGURED',
      });
    } finally {
      await redis.del(`presence:${device.id}`, `presence:${otherDevice.id}`);
      await connection.db
        .delete(characterProfiles)
        .where(inArray(characterProfiles.userId, [user.id, otherUser.id]));
      await connection.db.delete(roomSnapshots).where(
        inArray(
          roomSnapshots.roomId,
          insertedRooms.map((row) => row.id),
        ),
      );
      await connection.db
        .delete(executions)
        .where(eq(executions.desktopDeviceId, device.id));
      await connection.db
        .delete(decisions)
        .where(inArray(decisions.userId, [user.id, otherUser.id]));
      await connection.db.delete(proposals).where(
        inArray(
          proposals.roomId,
          insertedRooms.map((row) => row.id),
        ),
      );
      await connection.db.delete(commands).where(
        inArray(
          commands.roomId,
          insertedRooms.map((row) => row.id),
        ),
      );
      await connection.db.delete(rooms).where(
        inArray(
          rooms.id,
          insertedRooms.map((row) => row.id),
        ),
      );
      await connection.db.delete(devices).where(
        inArray(
          devices.id,
          insertedDevices.map((row) => row.id),
        ),
      );
      await connection.db.delete(users).where(
        inArray(
          users.id,
          insertedUsers.map((row) => row.id),
        ),
      );
    }
  });
});
