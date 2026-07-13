import {
  BadRequestException,
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
  createExecutionSchema,
  idempotencyKeySchema,
  updateExecutionSchema,
} from '@mousekeeper/contracts';
import { z } from 'zod';
import { CurrentPrincipal } from '../auth/auth-principal';
import { AgentOnly } from '../auth/agent-only.decorator';
import { requireAgentDevice } from '../auth/agent-device';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import { ZodValidationPipe } from '../common/zod-validation.pipe';
import { ExecutionsService } from './executions.service';
@Controller('v1')
@UseGuards(FirebaseAuthGuard)
export class ExecutionsController {
  constructor(private readonly executions: ExecutionsService) {}
  @Post('agent/executions')
  @AgentOnly()
  create(
    @CurrentPrincipal() p: AuthPrincipal,
    @Headers('idempotency-key') raw: string | undefined,
    @Body(new ZodValidationPipe(createExecutionSchema))
    body: z.infer<typeof createExecutionSchema>,
  ) {
    requireAgentDevice(p, body.desktopDeviceId);
    const key = idempotencyKeySchema.safeParse(raw);
    if (!key.success)
      throw new BadRequestException({ code: 'VALIDATION_FAILED' });
    return this.executions.create(p.userId, key.data, body);
  }
  @Patch('agent/executions/:id')
  @AgentOnly()
  update(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('id') id: string,
    @Headers('idempotency-key') raw: string | undefined,
    @Body(new ZodValidationPipe(updateExecutionSchema))
    body: z.infer<typeof updateExecutionSchema>,
  ) {
    const key = idempotencyKeySchema.safeParse(raw);
    if (!key.success)
      throw new BadRequestException({ code: 'VALIDATION_FAILED' });
    return this.executions.update(
      p.userId,
      requireAgentDevice(p),
      id,
      key.data,
      body,
    );
  }

  @Get('rooms/:roomId/executions')
  list(@CurrentPrincipal() p: AuthPrincipal, @Param('roomId') roomId: string) {
    return this.executions.listForRoom(p.userId, roomId);
  }

  @Get('executions/:id')
  get(@CurrentPrincipal() p: AuthPrincipal, @Param('id') id: string) {
    return this.executions.get(p.userId, id);
  }
}
