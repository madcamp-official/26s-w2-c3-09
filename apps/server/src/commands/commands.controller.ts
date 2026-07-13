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
  createCommandSchema,
  idempotencyKeySchema,
  updateCommandStatusSchema,
} from '@mousekeeper/contracts';
import { z } from 'zod';
import { CurrentPrincipal } from '../auth/auth-principal';
import { AgentOnly } from '../auth/agent-only.decorator';
import { requireAgentDevice } from '../auth/agent-device';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import { ZodValidationPipe } from '../common/zod-validation.pipe';
import { CommandsService } from './commands.service';

@Controller('v1')
@UseGuards(FirebaseAuthGuard)
export class CommandsController {
  constructor(private readonly commands: CommandsService) {}
  @Post('rooms/:roomId/commands')
  create(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('roomId') roomId: string,
    @Headers('idempotency-key') rawKey: string | undefined,
    @Body(new ZodValidationPipe(createCommandSchema))
    body: z.infer<typeof createCommandSchema>,
  ) {
    const key = idempotencyKeySchema.safeParse(rawKey);
    if (!key.success)
      throw new BadRequestException({
        code: 'VALIDATION_FAILED',
        message: 'Valid Idempotency-Key is required',
      });
    if (
      body.metadata?.idempotencyKey &&
      body.metadata.idempotencyKey !== key.data
    )
      throw new BadRequestException({
        code: 'VALIDATION_FAILED',
        message: 'Command metadata idempotencyKey must match the header',
      });
    return this.commands.create(p.userId, roomId, key.data, body);
  }
  @Get('devices/:deviceId/commands/pending')
  @AgentOnly()
  pending(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('deviceId') deviceId: string,
  ) {
    requireAgentDevice(p, deviceId);
    return this.commands.pending(p.userId, deviceId);
  }
  @Get('rooms/:roomId/commands')
  listForRoom(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('roomId') roomId: string,
  ) {
    return this.commands.listForRoom(p.userId, roomId);
  }
  @Patch('devices/:deviceId/commands/:commandId/status')
  @AgentOnly()
  update(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('deviceId') deviceId: string,
    @Param('commandId') commandId: string,
    @Body(new ZodValidationPipe(updateCommandStatusSchema))
    body: z.infer<typeof updateCommandStatusSchema>,
  ) {
    requireAgentDevice(p, deviceId);
    return this.commands.update(p.userId, deviceId, commandId, body);
  }
}
