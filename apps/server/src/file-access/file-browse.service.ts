import {
  ForbiddenException,
  Inject,
  Injectable,
  NotFoundException,
} from '@nestjs/common';
import {
  completeFileBrowseSchema,
  createFileBrowseRequestSchema,
  failFileBrowseSchema,
} from '@mousekeeper/contracts';
import {
  devices,
  fileBrowseRequests,
  rooms,
  type Database,
} from '@mousekeeper/database';
import { and, asc, eq, gt, inArray } from 'drizzle-orm';
import { z } from 'zod';
import Redis from 'ioredis';
import { DATABASE } from '../database/database.module';
import { REDIS } from '../presence/redis.module';
import { SyncService } from '../sync/sync.service';
@Injectable()
export class FileBrowseService {
  constructor(
    @Inject(DATABASE) private readonly db: Database,
    @Inject(REDIS) private readonly redis: Redis,
    private readonly sync: SyncService,
  ) {}
  async create(
    userId: string,
    roomId: string,
    body: z.infer<typeof createFileBrowseRequestSchema>,
  ) {
    if (this.redis.status === 'wait') await this.redis.connect();
    return this.db.transaction(async (tx) => {
      const candidate = (
        await tx
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
      if (!candidate) throw new NotFoundException({ code: 'NOT_FOUND' });
      const device = (
        await tx
          .select()
          .from(devices)
          .where(
            and(
              eq(devices.id, candidate.desktopDeviceId),
              eq(devices.userId, userId),
              eq(devices.status, 'ACTIVE'),
            ),
          )
          .for('share')
          .limit(1)
      )[0];
      if (!device) throw new NotFoundException({ code: 'NOT_FOUND' });
      const room = (
        await tx
          .select()
          .from(rooms)
          .where(
            and(
              eq(rooms.id, roomId),
              eq(rooms.desktopDeviceId, device.id),
              eq(rooms.userId, userId),
              eq(rooms.status, 'ACTIVE'),
            ),
          )
          .for('share')
          .limit(1)
      )[0];
      if (!room) throw new NotFoundException({ code: 'NOT_FOUND' });
      const presence = await this.redis.get(`presence:${room.desktopDeviceId}`);
      const request = (
        await tx
          .insert(fileBrowseRequests)
          .values({
            roomId,
            desktopDeviceId: room.desktopDeviceId,
            relativeDirectory: body.relativeDirectory,
            cursor: body.cursor,
            query: presence ? body.query : null,
            searchScope: body.searchScope,
            expiresAt: new Date(Date.now() + 60_000),
            ...(presence
              ? {}
              : { status: 'FAILED', failureCode: 'DEVICE_OFFLINE' }),
          })
          .returning()
      )[0];
      if (!request) throw new Error('Browse insert failed');
      await this.sync.append(tx, {
        userId,
        deviceId: room.desktopDeviceId,
        roomId,
        eventType: presence ? 'file.browse.requested' : 'file.browse.failed',
        aggregateType: 'file_browse_request',
        aggregateId: request.id,
        payload: {
          requestId: request.id,
          ...(presence ? {} : { failureCode: 'DEVICE_OFFLINE' }),
        },
      });
      return request;
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
    return this.db
      .select({ request: fileBrowseRequests })
      .from(fileBrowseRequests)
      .innerJoin(
        rooms,
        and(
          eq(fileBrowseRequests.roomId, rooms.id),
          eq(rooms.status, 'ACTIVE'),
        ),
      )
      .where(
        and(
          eq(fileBrowseRequests.desktopDeviceId, deviceId),
          eq(fileBrowseRequests.status, 'REQUESTED'),
          gt(fileBrowseRequests.expiresAt, new Date()),
        ),
      )
      .orderBy(asc(fileBrowseRequests.createdAt))
      .then((rows) => rows.map(({ request }) => request));
  }
  async get(userId: string, requestId: string) {
    const request = (
      await this.db
        .select({ request: fileBrowseRequests })
        .from(fileBrowseRequests)
        .innerJoin(
          rooms,
          and(
            eq(fileBrowseRequests.roomId, rooms.id),
            eq(rooms.userId, userId),
            eq(rooms.status, 'ACTIVE'),
          ),
        )
        .where(eq(fileBrowseRequests.id, requestId))
        .limit(1)
    )[0]?.request;
    if (!request) throw new NotFoundException({ code: 'NOT_FOUND' });
    if (request.expiresAt <= new Date()) {
      return this.db.transaction(async (tx) => {
        await this.lockActiveScope(
          tx,
          userId,
          request.desktopDeviceId,
          request.roomId,
        );
        if (request.status === 'FAILED') {
          const sanitized = (
            await tx
              .update(fileBrowseRequests)
              .set({ query: null, resultPage: null })
              .where(eq(fileBrowseRequests.id, requestId))
              .returning()
          )[0];
          if (!sanitized) throw new NotFoundException({ code: 'NOT_FOUND' });
          return sanitized;
        }
        const expired = (
          await tx
            .update(fileBrowseRequests)
            .set({
              status: 'FAILED',
              failureCode: 'TIMED_OUT',
              query: null,
              resultPage: null,
            })
            .where(
              and(
                eq(fileBrowseRequests.id, requestId),
                inArray(fileBrowseRequests.status, ['REQUESTED', 'READY']),
              ),
            )
            .returning()
        )[0];
        if (!expired) {
          const current = (
            await tx
              .select()
              .from(fileBrowseRequests)
              .where(eq(fileBrowseRequests.id, requestId))
              .limit(1)
          )[0];
          if (!current) throw new NotFoundException({ code: 'NOT_FOUND' });
          return current;
        }
        await this.sync.append(tx, {
          userId,
          deviceId: request.desktopDeviceId,
          roomId: request.roomId,
          eventType: 'file.browse.failed',
          aggregateType: 'file_browse_request',
          aggregateId: request.id,
          payload: { requestId: request.id, failureCode: 'TIMED_OUT' },
        });
        return expired;
      });
    }
    return request;
  }
  async complete(
    userId: string,
    deviceId: string,
    requestId: string,
    body: z.infer<typeof completeFileBrowseSchema>,
  ) {
    return this.finish(userId, deviceId, requestId, {
      status: 'READY',
      resultPage: { entries: body.entries, nextCursor: body.nextCursor },
      desktopGeneration: body.desktopGeneration,
    });
  }
  async fail(
    userId: string,
    deviceId: string,
    requestId: string,
    body: z.infer<typeof failFileBrowseSchema>,
  ) {
    return this.finish(userId, deviceId, requestId, {
      status: 'FAILED',
      failureCode: body.failureCode,
    });
  }
  private async finish(
    userId: string,
    deviceId: string,
    requestId: string,
    values: {
      status: string;
      resultPage?: unknown;
      desktopGeneration?: string;
      failureCode?: string;
    },
  ) {
    return this.db.transaction(async (tx) => {
      // Device -> room is the same lock order used by device revocation. This
      // prevents a late browse result from reviving work after disconnect.
      const device = (
        await tx
          .select()
          .from(devices)
          .where(
            and(
              eq(devices.id, deviceId),
              eq(devices.userId, userId),
              eq(devices.status, 'ACTIVE'),
            ),
          )
          .for('share')
          .limit(1)
      )[0];
      if (!device) throw new NotFoundException({ code: 'NOT_FOUND' });
      const candidate = (
        await tx
          .select()
          .from(fileBrowseRequests)
          .where(
            and(
              eq(fileBrowseRequests.id, requestId),
              eq(fileBrowseRequests.desktopDeviceId, deviceId),
            ),
          )
          .limit(1)
      )[0];
      if (!candidate) throw new NotFoundException({ code: 'NOT_FOUND' });
      const room = (
        await tx
          .select()
          .from(rooms)
          .where(
            and(
              eq(rooms.id, candidate.roomId),
              eq(rooms.desktopDeviceId, device.id),
              eq(rooms.userId, userId),
              eq(rooms.status, 'ACTIVE'),
            ),
          )
          .for('share')
          .limit(1)
      )[0];
      if (!room) throw new NotFoundException({ code: 'NOT_FOUND' });
      const owned = (
        await tx
          .select()
          .from(fileBrowseRequests)
          .where(
            and(
              eq(fileBrowseRequests.id, requestId),
              eq(fileBrowseRequests.desktopDeviceId, device.id),
              eq(fileBrowseRequests.roomId, room.id),
            ),
          )
          .for('update')
          .limit(1)
      )[0];
      if (!owned) throw new NotFoundException({ code: 'NOT_FOUND' });

      if (owned.expiresAt <= new Date()) {
        const expired = (
          await tx
            .update(fileBrowseRequests)
            .set({
              status: 'FAILED',
              failureCode: 'TIMED_OUT',
              query: null,
              resultPage: null,
            })
            .where(
              and(
                eq(fileBrowseRequests.id, requestId),
                eq(fileBrowseRequests.status, 'REQUESTED'),
              ),
            )
            .returning()
        )[0];
        if (!expired)
          throw new NotFoundException({ code: 'INVALID_STATE_TRANSITION' });
        await this.sync.append(tx, {
          userId,
          deviceId,
          roomId: owned.roomId,
          eventType: 'file.browse.failed',
          aggregateType: 'file_browse_request',
          aggregateId: requestId,
          payload: { requestId, failureCode: 'TIMED_OUT' },
        });
        return expired;
      }

      const updated = (
        await tx
          .update(fileBrowseRequests)
          .set({ ...values, query: null })
          .where(
            and(
              eq(fileBrowseRequests.id, requestId),
              eq(fileBrowseRequests.status, 'REQUESTED'),
            ),
          )
          .returning()
      )[0];
      if (!updated)
        throw new NotFoundException({ code: 'INVALID_STATE_TRANSITION' });
      await this.sync.append(tx, {
        userId,
        deviceId,
        roomId: owned.roomId,
        eventType:
          values.status === 'READY'
            ? 'file.browse.ready'
            : 'file.browse.failed',
        aggregateType: 'file_browse_request',
        aggregateId: requestId,
        payload: {
          requestId,
          status: values.status,
          ...(values.failureCode ? { failureCode: values.failureCode } : {}),
        },
      });
      return updated;
    });
  }

  private async lockActiveScope(
    tx: Parameters<Parameters<Database['transaction']>[0]>[0],
    userId: string,
    deviceId: string,
    roomId: string,
  ) {
    const device = (
      await tx
        .select()
        .from(devices)
        .where(
          and(
            eq(devices.id, deviceId),
            eq(devices.userId, userId),
            eq(devices.status, 'ACTIVE'),
          ),
        )
        .for('share')
        .limit(1)
    )[0];
    if (!device) throw new NotFoundException({ code: 'NOT_FOUND' });
    const room = (
      await tx
        .select()
        .from(rooms)
        .where(
          and(
            eq(rooms.id, roomId),
            eq(rooms.desktopDeviceId, device.id),
            eq(rooms.userId, userId),
            eq(rooms.status, 'ACTIVE'),
          ),
        )
        .for('share')
        .limit(1)
    )[0];
    if (!room) throw new NotFoundException({ code: 'NOT_FOUND' });
  }
}
