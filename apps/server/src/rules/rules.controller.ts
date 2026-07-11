import {
  Body,
  Controller,
  Get,
  Param,
  Patch,
  Post,
  UseGuards,
} from '@nestjs/common';
import { createRuleSchema, updateRuleSchema } from '@housemouse/contracts';
import { z } from 'zod';
import { CurrentPrincipal } from '../auth/auth-principal';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import { ZodValidationPipe } from '../common/zod-validation.pipe';
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
