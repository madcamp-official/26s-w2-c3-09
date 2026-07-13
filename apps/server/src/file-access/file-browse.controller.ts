import { Body, Controller, Get, Param, Post, UseGuards } from '@nestjs/common';
import {
  completeFileBrowseSchema,
  createFileBrowseRequestSchema,
  failFileBrowseSchema,
} from '@mousekeeper/contracts';
import { z } from 'zod';
import { CurrentPrincipal } from '../auth/auth-principal';
import { AgentOnly } from '../auth/agent-only.decorator';
import { requireAgentDevice } from '../auth/agent-device';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import { ZodValidationPipe } from '../common/zod-validation.pipe';
import { FileBrowseService } from './file-browse.service';
@Controller('v1')
@UseGuards(FirebaseAuthGuard)
export class FileBrowseController {
  constructor(private readonly browse: FileBrowseService) {}
  @Post('rooms/:roomId/file-browse-requests') create(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('roomId') roomId: string,
    @Body(new ZodValidationPipe(createFileBrowseRequestSchema))
    body: z.infer<typeof createFileBrowseRequestSchema>,
  ) {
    return this.browse.create(p.userId, roomId, body);
  }
  @Get('devices/:deviceId/file-browse-requests/pending')
  @AgentOnly()
  pending(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('deviceId') deviceId: string,
  ) {
    requireAgentDevice(p, deviceId);
    return this.browse.pending(p.userId, deviceId);
  }
  @Get('file-browse-requests/:id')
  get(@CurrentPrincipal() p: AuthPrincipal, @Param('id') id: string) {
    return this.browse.get(p.userId, id);
  }
  @Post('agent/file-browse-requests/:id/result')
  @AgentOnly()
  complete(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('id') id: string,
    @Body(new ZodValidationPipe(completeFileBrowseSchema))
    body: z.infer<typeof completeFileBrowseSchema>,
  ) {
    return this.browse.complete(p.userId, requireAgentDevice(p), id, body);
  }
  @Post('agent/file-browse-requests/:id/failure')
  @AgentOnly()
  fail(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('id') id: string,
    @Body(new ZodValidationPipe(failFileBrowseSchema))
    body: z.infer<typeof failFileBrowseSchema>,
  ) {
    return this.browse.fail(p.userId, requireAgentDevice(p), id, body);
  }
}
