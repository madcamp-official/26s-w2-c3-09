import { Inject, Injectable, UnauthorizedException } from '@nestjs/common';
import { devices, users, type Database } from '@housemouse/database';
import { and, eq } from 'drizzle-orm';
import { cert, getApps, initializeApp } from 'firebase-admin/app';
import { getAuth } from 'firebase-admin/auth';
import { verify } from 'jsonwebtoken';
import { loadEnvironment } from '../config/environment';
import { DATABASE } from '../database/database.module';
import type { AuthPrincipal } from './auth-principal';

@Injectable()
export class AuthService {
  constructor(@Inject(DATABASE) private readonly db: Database) {}

  async authenticate(idToken: string): Promise<AuthPrincipal> {
    try {
      const env = loadEnvironment();
      const app =
        getApps()[0] ??
        initializeApp({
          credential: cert({
            projectId: env.FIREBASE_PROJECT_ID,
            clientEmail: env.FIREBASE_CLIENT_EMAIL,
            privateKey: env.FIREBASE_PRIVATE_KEY.replace(/\\n/g, '\n'),
          }),
        });
      const decoded = await getAuth(app).verifyIdToken(idToken, true);
      const displayName =
        decoded.name?.trim() ||
        decoded.email?.split('@')[0] ||
        'HOUSEMOUSE 사용자';
      const existing = (
        await this.db
          .select()
          .from(users)
          .where(eq(users.authProviderUid, decoded.uid))
          .limit(1)
      )[0];
      const user =
        existing ??
        (
          await this.db
            .insert(users)
            .values({ authProviderUid: decoded.uid, displayName })
            .returning()
        )[0];
      if (!user) throw new Error('User upsert failed');
      return {
        userId: user.id,
        authProviderUid: user.authProviderUid,
        displayName: user.displayName,
        authType: 'FIREBASE',
        deviceId: null,
      };
    } catch {
      throw new UnauthorizedException({
        code: 'UNAUTHENTICATED',
        message: 'Firebase ID token is invalid',
      });
    }
  }

  async authenticateDevice(token: string): Promise<AuthPrincipal> {
    try {
      const payload = verify(
        token,
        loadEnvironment().JWT_OR_DEVICE_TOKEN_SECRET,
        { issuer: 'housemouse-server', audience: 'housemouse-desktop' },
      );
      if (typeof payload === 'string' || typeof payload.sub !== 'string')
        throw new Error('Invalid device token');
      const row = (
        await this.db
          .select({ device: devices, user: users })
          .from(devices)
          .innerJoin(users, eq(devices.userId, users.id))
          .where(and(eq(devices.id, payload.sub), eq(devices.status, 'ACTIVE')))
          .limit(1)
      )[0];
      if (!row) throw new Error('Device revoked');
      return {
        userId: row.user.id,
        authProviderUid: row.user.authProviderUid,
        displayName: row.user.displayName,
        authType: 'DEVICE',
        deviceId: row.device.id,
      };
    } catch {
      throw new UnauthorizedException({
        code: 'UNAUTHENTICATED',
        message: 'Device token is invalid or revoked',
      });
    }
  }
}
