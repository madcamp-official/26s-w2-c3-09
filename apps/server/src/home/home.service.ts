import { Inject, Injectable } from '@nestjs/common';
import { homeSummaryResponseSchema } from '@mousekeeper/contracts';
import {
  devices,
  executions,
  proposals,
  roomSnapshots,
  rooms,
  type Database,
} from '@mousekeeper/database';
import { and, asc, count, desc, eq } from 'drizzle-orm';
import Redis from 'ioredis';
import { CharacterService } from '../character/character.service';
import { DATABASE } from '../database/database.module';
import { REDIS } from '../presence/redis.module';

@Injectable()
export class HomeService {
  constructor(
    @Inject(DATABASE) private readonly db: Database,
    @Inject(REDIS) private readonly redis: Redis,
    private readonly character: CharacterService,
  ) {}

  async summary(userId: string) {
    const [
      deviceRows,
      roomRows,
      proposalCounts,
      latestExecutions,
      snapshots,
      character,
    ] = await Promise.all([
      this.db
        .select()
        .from(devices)
        .where(and(eq(devices.userId, userId), eq(devices.status, 'ACTIVE')))
        .orderBy(asc(devices.createdAt)),
      this.db
        .select()
        .from(rooms)
        .where(and(eq(rooms.userId, userId), eq(rooms.status, 'ACTIVE')))
        .orderBy(asc(rooms.createdAt)),
      this.db
        .select({
          roomId: proposals.roomId,
          pendingProposalCount: count(proposals.id),
        })
        .from(proposals)
        .innerJoin(
          rooms,
          and(
            eq(rooms.id, proposals.roomId),
            eq(rooms.userId, userId),
            eq(rooms.status, 'ACTIVE'),
          ),
        )
        .where(eq(proposals.status, 'OPEN'))
        .groupBy(proposals.roomId),
      this.db
        .selectDistinctOn([proposals.roomId], {
          roomId: proposals.roomId,
          status: executions.status,
        })
        .from(executions)
        .innerJoin(proposals, eq(proposals.id, executions.proposalId))
        .innerJoin(
          rooms,
          and(
            eq(rooms.id, proposals.roomId),
            eq(rooms.userId, userId),
            eq(rooms.status, 'ACTIVE'),
          ),
        )
        .orderBy(
          proposals.roomId,
          desc(executions.startedAt),
          desc(executions.id),
        ),
      this.db
        .selectDistinctOn([roomSnapshots.roomId], {
          roomId: roomSnapshots.roomId,
          score: roomSnapshots.score,
          formulaVersion: roomSnapshots.formulaVersion,
          calculatedAt: roomSnapshots.calculatedAt,
        })
        .from(roomSnapshots)
        .innerJoin(
          rooms,
          and(
            eq(rooms.id, roomSnapshots.roomId),
            eq(rooms.userId, userId),
            eq(rooms.status, 'ACTIVE'),
          ),
        )
        .orderBy(
          roomSnapshots.roomId,
          desc(roomSnapshots.calculatedAt),
          desc(roomSnapshots.id),
        ),
      this.character.get(userId),
    ]);

    if (this.redis.status === 'wait') await this.redis.connect();
    const presenceValues = deviceRows.length
      ? await this.redis.mget(
          ...deviceRows.map((device) => `presence:${device.id}`),
        )
      : [];
    const proposalCountByRoom = new Map(
      proposalCounts.map((row) => [row.roomId, row.pendingProposalCount]),
    );
    const executionStatusByRoom = new Map(
      latestExecutions.map((row) => [row.roomId, row.status]),
    );
    const snapshotByRoom = new Map(
      snapshots.map((snapshot) => [snapshot.roomId, snapshot]),
    );

    return homeSummaryResponseSchema.parse({
      devices: deviceRows.map((device, index) => ({
        id: device.id,
        platform: device.platform,
        deviceName: device.deviceName,
        status: device.status,
        lastSeenAt: device.lastSeenAt?.toISOString() ?? null,
        createdAt: device.createdAt.toISOString(),
        presence: presenceValues[index] ?? 'OFFLINE',
      })),
      rooms: roomRows.map((room) => {
        const snapshot = snapshotByRoom.get(room.id);
        return {
          id: room.id,
          desktopDeviceId: room.desktopDeviceId,
          name: room.name,
          rootAlias: room.rootAlias,
          status: room.status,
          createdAt: room.createdAt.toISOString(),
          pendingProposalCount: proposalCountByRoom.get(room.id) ?? 0,
          latestExecutionStatus: executionStatusByRoom.get(room.id) ?? null,
          cleanlinessScore: snapshot?.score ?? null,
          cleanlinessFormulaVersion: snapshot?.formulaVersion ?? null,
          cleanlinessCalculatedAt: snapshot?.calculatedAt.toISOString() ?? null,
        };
      }),
      character,
    });
  }
}
