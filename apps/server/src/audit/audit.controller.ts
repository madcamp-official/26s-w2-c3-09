import { Controller, Get, Param, Query, UseGuards } from '@nestjs/common';
import { z } from 'zod';
import { CurrentPrincipal } from '../auth/auth-principal';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import { AuditService } from './audit.service';

const querySchema = z.object({
  limit: z.coerce.number().int().min(1).max(100).default(30),
});

@Controller('v1/rooms/:roomId/activity')
@UseGuards(FirebaseAuthGuard)
export class AuditController {
  constructor(private readonly audit: AuditService) {}

  @Get()
  list(
    @CurrentPrincipal() principal: AuthPrincipal,
    @Param('roomId') roomId: string,
    @Query() raw: unknown,
  ) {
    const query = querySchema.parse(raw);
    return this.audit.forRoom(principal.userId, roomId, query.limit);
  }
}
