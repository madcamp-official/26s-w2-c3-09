import {
  BadRequestException,
  Body,
  Controller,
  Delete,
  Get,
  Headers,
  Param,
  Post,
  UseGuards,
} from '@nestjs/common';
import {
  completeFileUploadSchema,
  createFileTransferSchema,
  failFileTransferSchema,
  idempotencyKeySchema,
  requestUploadTargetSchema,
} from '@housemouse/contracts';
import { z } from 'zod';
import { CurrentPrincipal } from '../auth/auth-principal';
import { AgentOnly } from '../auth/agent-only.decorator';
import { requireAgentDevice } from '../auth/agent-device';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import { ZodValidationPipe } from '../common/zod-validation.pipe';
import { TransfersService } from './transfers.service';
@Controller('v1')
@UseGuards(FirebaseAuthGuard)
export class TransfersController {
  constructor(private readonly transfers: TransfersService) {}
  @Post('rooms/:roomId/file-transfers') create(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('roomId') roomId: string,
    @Headers('idempotency-key') raw: string | undefined,
    @Body(new ZodValidationPipe(createFileTransferSchema))
    body: z.infer<typeof createFileTransferSchema>,
  ) {
    const key = idempotencyKeySchema.safeParse(raw);
    if (!key.success)
      throw new BadRequestException({ code: 'VALIDATION_FAILED' });
    return this.transfers.create(p.userId, roomId, key.data, body);
  }
  @Get('devices/:deviceId/file-transfers/pending')
  @AgentOnly()
  pending(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('deviceId') deviceId: string,
  ) {
    requireAgentDevice(p, deviceId);
    return this.transfers.pending(p.userId, deviceId);
  }
  @Post('agent/file-transfers/:id/upload-target')
  @AgentOnly()
  upload(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('id') id: string,
    @Body(new ZodValidationPipe(requestUploadTargetSchema))
    body: z.infer<typeof requestUploadTargetSchema>,
  ) {
    return this.transfers.uploadTarget(
      p.userId,
      requireAgentDevice(p),
      id,
      body,
    );
  }
  @Post('agent/file-transfers/:id/complete-upload')
  @AgentOnly()
  complete(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('id') id: string,
    @Headers('idempotency-key') raw: string | undefined,
    @Body(new ZodValidationPipe(completeFileUploadSchema))
    body: z.infer<typeof completeFileUploadSchema>,
  ) {
    const key = idempotencyKeySchema.safeParse(raw);
    if (!key.success)
      throw new BadRequestException({ code: 'VALIDATION_FAILED' });
    return this.transfers.complete(
      p.userId,
      requireAgentDevice(p),
      id,
      key.data,
      body,
    );
  }
  @Post('agent/file-transfers/:id/failure')
  @AgentOnly()
  fail(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('id') id: string,
    @Body(new ZodValidationPipe(failFileTransferSchema))
    body: z.infer<typeof failFileTransferSchema>,
  ) {
    return this.transfers.fail(p.userId, requireAgentDevice(p), id, body);
  }
  @Get('file-transfers/:id')
  get(@CurrentPrincipal() p: AuthPrincipal, @Param('id') id: string) {
    return this.transfers.get(p.userId, id);
  }
  @Get('file-transfers/:id/download') download(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('id') id: string,
  ) {
    return this.transfers.download(p.userId, id);
  }
  @Post('file-transfers/:id/ack') ack(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('id') id: string,
  ) {
    return this.transfers.ack(p.userId, id);
  }

  @Delete('file-transfers/:id')
  cancel(@CurrentPrincipal() p: AuthPrincipal, @Param('id') id: string) {
    return this.transfers.cancel(p.userId, p.deviceId ?? undefined, id);
  }
}
