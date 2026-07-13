import {
  Controller,
  Delete,
  ForbiddenException,
  Get,
  Headers,
  Inject,
  Param,
  UseGuards,
} from '@nestjs/common';
import { devices, type Database } from '@mousekeeper/database';
import { and, eq } from 'drizzle-orm';
import { CurrentPrincipal } from '../auth/auth-principal';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import {
  ConnectionLifecycleService,
  mobileConnectionActor,
} from '../connections/connection-lifecycle.service';
import { requireIdempotencyKey } from '../connections/idempotency-key';
import { DATABASE } from '../database/database.module';

@Controller('v1/devices')
@UseGuards(FirebaseAuthGuard)
export class DevicesController {
  constructor(
    @Inject(DATABASE) private readonly db: Database,
    private readonly lifecycle: ConnectionLifecycleService,
  ) {}

  @Get()
  async list(@CurrentPrincipal() principal: AuthPrincipal) {
    const result = await this.db
      .select()
      .from(devices)
      .where(
        and(eq(devices.userId, principal.userId), eq(devices.status, 'ACTIVE')),
      );
    return result.map((device) => this.publicDevice(device));
  }

  @Delete(':id')
  revoke(
    @CurrentPrincipal() principal: AuthPrincipal,
    @Param('id') id: string,
    @Headers('idempotency-key') rawKey: string | undefined,
  ) {
    if (principal.authType !== 'FIREBASE') {
      throw new ForbiddenException({ code: 'FORBIDDEN' });
    }
    return this.lifecycle.revokeDevice(
      mobileConnectionActor(principal.userId),
      id,
      requireIdempotencyKey(rawKey),
    );
  }

  private publicDevice(device: typeof devices.$inferSelect) {
    const { userId: _, publicKey: __, ...safe } = device;
    return safe;
  }
}
