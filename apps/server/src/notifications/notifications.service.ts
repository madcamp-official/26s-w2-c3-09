import { createHash } from 'node:crypto';
import { Inject, Injectable, NotFoundException } from '@nestjs/common';
import { pushNotificationTokens, type Database } from '@housemouse/database';
import { and, eq } from 'drizzle-orm';
import { DATABASE } from '../database/database.module';

type RegisterToken = { token: string; platform: 'ANDROID' | 'IOS' };

@Injectable()
export class NotificationsService {
  constructor(@Inject(DATABASE) private readonly db: Database) {}

  async register(userId: string, input: RegisterToken) {
    const tokenHash = createHash('sha256').update(input.token).digest('hex');
    const token = (
      await this.db
        .insert(pushNotificationTokens)
        .values({
          userId,
          token: input.token,
          tokenHash,
          platform: input.platform,
        })
        .onConflictDoUpdate({
          target: pushNotificationTokens.tokenHash,
          set: {
            userId,
            token: input.token,
            platform: input.platform,
            status: 'ACTIVE',
            lastSeenAt: new Date(),
            revokedAt: null,
          },
        })
        .returning()
    )[0];
    return this.publicToken(token);
  }

  async revoke(userId: string, id: string) {
    const token = (
      await this.db
        .update(pushNotificationTokens)
        .set({ status: 'REVOKED', revokedAt: new Date() })
        .where(
          and(
            eq(pushNotificationTokens.id, id),
            eq(pushNotificationTokens.userId, userId),
            eq(pushNotificationTokens.status, 'ACTIVE'),
          ),
        )
        .returning()
    )[0];
    if (!token) throw new NotFoundException({ code: 'NOT_FOUND' });
    return this.publicToken(token);
  }

  private publicToken(token: typeof pushNotificationTokens.$inferSelect) {
    return {
      id: token.id,
      platform: token.platform,
      status: token.status,
      lastSeenAt: token.lastSeenAt,
      createdAt: token.createdAt,
      revokedAt: token.revokedAt,
    };
  }
}
