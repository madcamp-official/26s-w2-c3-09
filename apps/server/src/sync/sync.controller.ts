import { Controller, Get, Query, UseGuards } from '@nestjs/common';
import { type Database } from '@housemouse/database';
import { Inject } from '@nestjs/common';
import { z } from 'zod';
import { CurrentPrincipal } from '../auth/auth-principal';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import { DATABASE } from '../database/database.module';
import { SyncService } from './sync.service';

const querySchema = z.object({
  after: z.coerce.number().int().nonnegative().default(0),
  limit: z.coerce.number().int().min(1).max(200).default(100),
});

@Controller('v1/sync')
@UseGuards(FirebaseAuthGuard)
export class SyncController {
  constructor(
    @Inject(DATABASE) private readonly db: Database,
    private readonly sync: SyncService,
  ) {}
  @Get('events')
  replay(@CurrentPrincipal() principal: AuthPrincipal, @Query() raw: unknown) {
    const query = querySchema.parse(raw);
    return this.sync.replay(
      this.db,
      principal.userId,
      query.after,
      query.limit,
    );
  }
}
