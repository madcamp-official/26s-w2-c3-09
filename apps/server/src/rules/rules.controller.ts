import {
  Body,
  Controller,
  Get,
  Headers,
  Param,
  Patch,
  Post,
  UseGuards,
} from '@nestjs/common';
import {
  confirmRuleDraftSchema,
  createRuleDraftRequestSchema,
  createRuleSchema,
  rejectRuleDraftSchema,
  updateRuleSchema,
} from '@mousekeeper/contracts';
import { z } from 'zod';
import { CurrentPrincipal } from '../auth/auth-principal';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import { ZodValidationPipe } from '../common/zod-validation.pipe';
import { requireIdempotencyKey } from '../connections/idempotency-key';
import { RulesService } from './rules.service';

@Controller('v1')
@UseGuards(FirebaseAuthGuard)
export class RulesController {
  constructor(private readonly rules: RulesService) {}

  @Get('rooms/:roomId/rules')
  list(
    @CurrentPrincipal() principal: AuthPrincipal,
    @Param('roomId') roomId: string,
  ) {
    return this.rules.list(principal.userId, roomId);
  }

  @Post('rooms/:roomId/rules')
  create(
    @CurrentPrincipal() principal: AuthPrincipal,
    @Param('roomId') roomId: string,
    @Body(new ZodValidationPipe(createRuleSchema))
    body: z.infer<typeof createRuleSchema>,
  ) {
    return this.rules.create(principal.userId, roomId, body);
  }

  @Post('rooms/:roomId/rule-drafts')
  createDraft(
    @CurrentPrincipal() principal: AuthPrincipal,
    @Param('roomId') roomId: string,
    @Body(new ZodValidationPipe(createRuleDraftRequestSchema))
    body: z.infer<typeof createRuleDraftRequestSchema>,
  ) {
    return this.rules.createDraft(principal.userId, roomId, body);
  }

  @Post('rule-drafts/:draftId/confirm')
  confirmDraft(
    @CurrentPrincipal() principal: AuthPrincipal,
    @Param('draftId') draftId: string,
    @Headers('idempotency-key') rawKey: string | undefined,
    @Body(new ZodValidationPipe(confirmRuleDraftSchema))
    _body: z.infer<typeof confirmRuleDraftSchema>,
  ) {
    const key = requireIdempotencyKey(rawKey);
    return this.rules.confirmDraft(principal.userId, draftId, key);
  }

  @Post('rule-drafts/:draftId/reject')
  rejectDraft(
    @CurrentPrincipal() principal: AuthPrincipal,
    @Param('draftId') draftId: string,
    @Body(new ZodValidationPipe(rejectRuleDraftSchema))
    _body: z.infer<typeof rejectRuleDraftSchema>,
  ) {
    return this.rules.rejectDraft(principal.userId, draftId);
  }

  @Patch('rules/:ruleId')
  update(
    @CurrentPrincipal() principal: AuthPrincipal,
    @Param('ruleId') ruleId: string,
    @Body(new ZodValidationPipe(updateRuleSchema))
    body: z.infer<typeof updateRuleSchema>,
  ) {
    return this.rules.update(principal.userId, ruleId, body);
  }
}
