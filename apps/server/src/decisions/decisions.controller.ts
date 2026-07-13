import {
  BadRequestException,
  Body,
  Controller,
  Get,
  Headers,
  Param,
  Post,
  UseGuards,
} from '@nestjs/common';
import {
  createDecisionSchema,
  idempotencyKeySchema,
} from '@mousekeeper/contracts';
import { z } from 'zod';
import { CurrentPrincipal } from '../auth/auth-principal';
import { AgentOnly } from '../auth/agent-only.decorator';
import { requireAgentDevice } from '../auth/agent-device';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import { ZodValidationPipe } from '../common/zod-validation.pipe';
import { DecisionsService } from './decisions.service';
@Controller('v1')
@UseGuards(FirebaseAuthGuard)
export class DecisionsController {
  constructor(private readonly decisions: DecisionsService) {}
  @Post('proposals/:id/decisions') create(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('id') id: string,
    @Headers('idempotency-key') raw: string | undefined,
    @Body(new ZodValidationPipe(createDecisionSchema))
    body: z.infer<typeof createDecisionSchema>,
  ) {
    const key = idempotencyKeySchema.safeParse(raw);
    if (!key.success)
      throw new BadRequestException({ code: 'VALIDATION_FAILED' });
    return this.decisions.create(p.userId, id, key.data, body);
  }

  @Get('devices/:deviceId/decisions/pending')
  @AgentOnly()
  pending(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('deviceId') deviceId: string,
  ) {
    requireAgentDevice(p, deviceId);
    return this.decisions.pending(p.userId, deviceId);
  }
}
