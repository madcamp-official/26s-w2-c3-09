import {
  ConflictException,
  Inject,
  Injectable,
  NotFoundException,
} from '@nestjs/common';
import {
  createPairingSessionSchema,
  devicePairedEventPayloadSchema,
} from '@mousekeeper/contracts';
import { devices, pairingSessions, type Database } from '@mousekeeper/database';
import { createHmac, randomBytes, randomInt } from 'node:crypto';
import { and, eq, gt, isNull } from 'drizzle-orm';
import { z } from 'zod';
import { sign } from 'jsonwebtoken';
import { DATABASE } from '../database/database.module';
import { loadEnvironment } from '../config/environment';
import { SyncService } from '../sync/sync.service';

export const hashPairingCode = (code: string, secret: string) =>
  createHmac('sha256', secret).update(code).digest('hex');

const hashCode = (code: string) =>
  hashPairingCode(code, loadEnvironment().JWT_OR_DEVICE_TOKEN_SECRET);

@Injectable()
export class PairingService {
  constructor(
    @Inject(DATABASE) private readonly db: Database,
    private readonly sync: SyncService,
  ) {}
  async create(body: z.infer<typeof createPairingSessionSchema>) {
    const code = randomInt(0, 1_000_000).toString().padStart(6, '0');
    const desktopNonce = randomBytes(32).toString('base64url');
    const session = (
      await this.db
        .insert(pairingSessions)
        .values({
          desktopNonce,
          pairingCodeHash: hashCode(code),
          deviceName: body.deviceName,
          platform: body.platform,
          publicKey: body.publicKey,
          expiresAt: new Date(Date.now() + 10 * 60 * 1000),
        })
        .returning()
    )[0];
    if (!session) throw new ConflictException({ code: 'CONFLICT' });
    return {
      sessionId: session.id,
      desktopNonce,
      code,
      expiresAt: session.expiresAt,
    };
  }
  async claim(userId: string, code: string) {
    return this.db.transaction(async (tx) => {
      const session = (
        await tx
          .select()
          .from(pairingSessions)
          .where(
            and(
              eq(pairingSessions.pairingCodeHash, hashCode(code)),
              isNull(pairingSessions.claimedAt),
              gt(pairingSessions.expiresAt, new Date()),
            ),
          )
          .for('update')
          .limit(1)
      )[0];
      if (!session)
        throw new NotFoundException({
          code: 'NOT_FOUND',
          message: 'Pairing code is invalid or expired',
        });
      const device = (
        await tx
          .insert(devices)
          .values({
            userId,
            platform: session.platform,
            deviceName: session.deviceName,
            publicKey: session.publicKey,
          })
          .returning()
      )[0];
      if (!device) throw new ConflictException({ code: 'CONFLICT' });
      await tx
        .update(pairingSessions)
        .set({
          claimedByUserId: userId,
          claimedDeviceId: device.id,
          claimedAt: new Date(),
        })
        .where(eq(pairingSessions.id, session.id));
      await this.sync.append(tx, {
        userId,
        deviceId: device.id,
        roomId: null,
        eventType: 'device.paired',
        aggregateType: 'device',
        aggregateId: device.id,
        payload: devicePairedEventPayloadSchema.parse({
          deviceId: device.id,
          status: 'ACTIVE',
        }),
      });
      return device;
    });
  }

  async status(sessionId: string, desktopNonce: string) {
    const session = (
      await this.db
        .select()
        .from(pairingSessions)
        .where(
          and(
            eq(pairingSessions.id, sessionId),
            eq(pairingSessions.desktopNonce, desktopNonce),
            gt(pairingSessions.expiresAt, new Date()),
          ),
        )
        .limit(1)
    )[0];
    if (!session) throw new NotFoundException({ code: 'NOT_FOUND' });
    if (!session.claimedDeviceId || !session.claimedByUserId)
      return { status: 'PENDING' as const, expiresAt: session.expiresAt };
    const token = sign({}, loadEnvironment().JWT_OR_DEVICE_TOKEN_SECRET, {
      subject: session.claimedDeviceId,
      issuer: 'mousekeeper-server',
      audience: 'mousekeeper-desktop',
      expiresIn: '90d',
    });
    return {
      status: 'CLAIMED' as const,
      deviceId: session.claimedDeviceId,
      deviceToken: `mk_device_${token}`,
    };
  }
}
