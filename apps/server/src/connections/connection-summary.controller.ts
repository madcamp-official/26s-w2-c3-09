import { Controller, ForbiddenException, Get, UseGuards } from '@nestjs/common';
import { CurrentPrincipal } from '../auth/auth-principal';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import { ConnectionSummaryService } from './connection-summary.service';

@Controller('v1/connections')
@UseGuards(FirebaseAuthGuard)
export class ConnectionSummaryController {
  constructor(private readonly connections: ConnectionSummaryService) {}

  @Get('summary')
  summary(@CurrentPrincipal() principal: AuthPrincipal) {
    if (principal.authType !== 'FIREBASE') {
      throw new ForbiddenException({
        code: 'FORBIDDEN',
        message: 'Firebase user token required',
      });
    }
    return this.connections.summary(principal.userId);
  }
}
