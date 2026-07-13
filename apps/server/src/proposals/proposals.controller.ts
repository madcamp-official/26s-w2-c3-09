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
  createProposalSchema,
  idempotencyKeySchema,
} from '@mousekeeper/contracts';
import { z } from 'zod';
import { CurrentPrincipal } from '../auth/auth-principal';
import { AgentOnly } from '../auth/agent-only.decorator';
import { requireAgentDevice } from '../auth/agent-device';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import { ZodValidationPipe } from '../common/zod-validation.pipe';
import { ProposalsService } from './proposals.service';
@Controller('v1')
@UseGuards(FirebaseAuthGuard)
export class ProposalsController {
  constructor(private readonly proposals: ProposalsService) {}
  @Post('agent/proposals')
  @AgentOnly()
  create(
    @CurrentPrincipal() p: AuthPrincipal,
    @Headers('idempotency-key') raw: string | undefined,
    @Body(new ZodValidationPipe(createProposalSchema))
    body: z.infer<typeof createProposalSchema>,
  ) {
    const key = idempotencyKeySchema.safeParse(raw);
    if (!key.success)
      throw new BadRequestException({ code: 'VALIDATION_FAILED' });
    return this.proposals.create(
      p.userId,
      requireAgentDevice(p),
      key.data,
      body,
    );
  }
  @Get('proposals/:id') get(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('id') id: string,
  ) {
    return this.proposals.get(p.userId, id);
  }
  @Get('rooms/:roomId/proposals/open')
  open(@CurrentPrincipal() p: AuthPrincipal, @Param('roomId') roomId: string) {
    return this.proposals.openForRoom(p.userId, roomId);
  }
}
