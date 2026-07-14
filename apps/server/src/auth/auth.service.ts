import { Inject, Injectable, UnauthorizedException } from '@nestjs/common';
import { devices, users, type Database } from '@mousekeeper/database';
import { eq } from 'drizzle-orm';
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
      const existingApp = getApps()[0];
      const app =
        existingApp ??
        (() => {
          const env = loadEnvironment();
          return initializeApp({
            credential: cert({
              projectId: env.FIREBASE_PROJECT_ID,
              clientEmail: env.FIREBASE_CLIENT_EMAIL,
              privateKey: env.FIREBASE_PRIVATE_KEY.replace(/\\n/g, '\n'),
            }),
          });
        })();
      const decoded: unknown = await getAuth(app).verifyIdToken(idToken, true);
      if (typeof decoded !== 'object' || decoded === null) {
        throw new Error('Firebase token payload is invalid');
      }
      const uid =
        'uid' in decoded && typeof decoded.uid === 'string'
          ? decoded.uid
          : null;
      if (!uid) throw new Error('Firebase token subject is missing');
      const name =
        'name' in decoded && typeof decoded.name === 'string'
          ? decoded.name.trim()
          : '';
      const email =
        'email' in decoded && typeof decoded.email === 'string'
          ? decoded.email
          : '';
      const displayName = name || email.split('@')[0] || 'MOUSEKEEPER 사용자';
      const existing = (
        await this.db
          .select()
          .from(users)
          .where(eq(users.authProviderUid, uid))
          .limit(1)
      )[0];
      const user =
        existing ??
        (
          await this.db
            .insert(users)
            .values({ authProviderUid: uid, displayName })
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
    const resolved = await this.resolveDeviceToken(token);
    if (resolved.status !== 'ACTIVE') {
      throw new UnauthorizedException({
        code: 'DEVICE_REVOKED',
        message: 'Desktop device pairing has been revoked',
      });
    }
    return resolved.principal;
  }

  async authenticateDeviceForRevocation(token: string): Promise<AuthPrincipal> {
    return (await this.resolveDeviceToken(token)).principal;
  }

  private async resolveDeviceToken(token: string): Promise<{
    principal: AuthPrincipal;
    status: string;
  }> {
    let subject: string;
    try {
      const payload = verify(
        token,
        loadEnvironment().JWT_OR_DEVICE_TOKEN_SECRET,
        { issuer: 'mousekeeper-server', audience: 'mousekeeper-desktop' },
      );
      if (typeof payload === 'string' || typeof payload.sub !== 'string')
        throw new Error('Invalid device token');
      subject = payload.sub;
    } catch {
      throw this.invalidDeviceToken();
    }

    const row = (
      await this.db
        .select({ device: devices, user: users })
        .from(devices)
        .innerJoin(users, eq(devices.userId, users.id))
        .where(eq(devices.id, subject))
        .limit(1)
    )[0];
    if (!row) {
      throw new UnauthorizedException({
        code: 'DEVICE_NOT_REGISTERED',
        message: 'Desktop device pairing no longer exists',
      });
    }
    return {
      status: row.device.status,
      principal: {
        userId: row.user.id,
        authProviderUid: row.user.authProviderUid,
        displayName: row.user.displayName,
        authType: 'DEVICE',
        deviceId: row.device.id,
      },
    };
  }

  private invalidDeviceToken() {
    return new UnauthorizedException({
      code: 'UNAUTHENTICATED',
      message: 'Device token is invalid',
    });
  }
}
