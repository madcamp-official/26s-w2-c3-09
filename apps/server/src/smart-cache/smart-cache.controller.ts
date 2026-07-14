import {
  BadRequestException,
  Body,
  Controller,
  Delete,
  Get,
  Headers,
  Param,
  Patch,
  Post,
  UseGuards,
} from '@nestjs/common';
import {
  cacheCandidateBatchSchema,
  cachedFileAccessEventSchema,
  completeCacheUploadSchema,
  idempotencyKeySchema,
  markCachedFilesStaleSchema,
  updateSmartCachePolicySchema,
} from '@mousekeeper/contracts';
import { z } from 'zod';
import { AgentOnly } from '../auth/agent-only.decorator';
import { requireAgentDevice } from '../auth/agent-device';
import { CurrentPrincipal } from '../auth/auth-principal';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import { ZodValidationPipe } from '../common/zod-validation.pipe';
import { SmartCacheService } from './smart-cache.service';
@Controller('v1')
@UseGuards(FirebaseAuthGuard)
export class SmartCacheController {
  constructor(private readonly cache: SmartCacheService) {}
  @Get('rooms/:roomId/smart-cache-policy') policy(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('roomId') roomId: string,
  ) {
    return this.cache.getPolicy(p.userId, roomId);
  }
  @Patch('rooms/:roomId/smart-cache-policy') update(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('roomId') roomId: string,
    @Body(new ZodValidationPipe(updateSmartCachePolicySchema))
    body: z.infer<typeof updateSmartCachePolicySchema>,
  ) {
    return this.cache.updatePolicy(p.userId, roomId, body);
  }
  @Get('rooms/:roomId/cached-files') list(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('roomId') roomId: string,
  ) {
    return this.cache.list(p.userId, roomId);
  }

  @Get('rooms/:roomId/smart-cache/files')
  listSmartCacheFiles(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('roomId') roomId: string,
  ) {
    return this.cache.list(p.userId, roomId);
  }

  @Post('agent/cache-candidates') @AgentOnly() submit(
    @CurrentPrincipal() p: AuthPrincipal,
    @Headers('idempotency-key') raw: string | undefined,
    @Body(new ZodValidationPipe(cacheCandidateBatchSchema))
    body: z.infer<typeof cacheCandidateBatchSchema>,
  ) {
    const key = idempotencyKeySchema.safeParse(raw);
    if (!key.success)
      throw new BadRequestException({ code: 'VALIDATION_FAILED' });
    return this.cache.submit(p.userId, requireAgentDevice(p), key.data, body);
  }
  @Post('agent/cache-uploads/:reservationId/complete') @AgentOnly() complete(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('reservationId') id: string,
    @Headers('idempotency-key') raw: string | undefined,
    @Body(new ZodValidationPipe(completeCacheUploadSchema))
    body: z.infer<typeof completeCacheUploadSchema>,
  ) {
    const key = idempotencyKeySchema.safeParse(raw);
    if (!key.success)
      throw new BadRequestException({ code: 'VALIDATION_FAILED' });
    return this.cache.complete(
      p.userId,
      requireAgentDevice(p),
      id,
      key.data,
      body,
    );
  }

  @Delete('agent/cache-uploads/:reservationId')
  @AgentOnly()
  cancel(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('reservationId') id: string,
  ) {
    return this.cache.cancelReservation(p.userId, requireAgentDevice(p), id);
  }

  @Post('agent/cached-files/stale')
  @AgentOnly()
  markStale(
    @CurrentPrincipal() p: AuthPrincipal,
    @Headers('idempotency-key') raw: string | undefined,
    @Body(new ZodValidationPipe(markCachedFilesStaleSchema))
    body: z.infer<typeof markCachedFilesStaleSchema>,
  ) {
    const key = idempotencyKeySchema.safeParse(raw);
    if (!key.success)
      throw new BadRequestException({ code: 'VALIDATION_FAILED' });
    return this.cache.markStale(p.userId, requireAgentDevice(p), body);
  }

  @Delete('cached-files/:cachedFileId')
  remove(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('cachedFileId') id: string,
  ) {
    return this.cache.remove(p.userId, id);
  }

  @Get('cached-files/:cachedFileId/download')
  download(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('cachedFileId') id: string,
  ) {
    return this.cache.download(p.userId, id);
  }

  @Post('cached-files/:cachedFileId/access-events')
  recordAccess(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('cachedFileId') id: string,
    @Body(new ZodValidationPipe(cachedFileAccessEventSchema))
    body: z.infer<typeof cachedFileAccessEventSchema>,
  ) {
    return this.cache.recordAccess(p.userId, id, body);
  }
}
