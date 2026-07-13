import { Body, Controller, Get, Param, Post, UseGuards } from '@nestjs/common';
import { createRoomSnapshotSchema } from '@mousekeeper/contracts';
import { z } from 'zod';
import { AgentOnly } from '../auth/agent-only.decorator';
import { requireAgentDevice } from '../auth/agent-device';
import { CurrentPrincipal } from '../auth/auth-principal';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import { ZodValidationPipe } from '../common/zod-validation.pipe';
import { SnapshotsService } from './snapshots.service';

@Controller('v1/rooms/:roomId/snapshots')
@UseGuards(FirebaseAuthGuard)
export class SnapshotsController {
  constructor(private readonly snapshots: SnapshotsService) {}

  @Get('latest')
  latest(
    @CurrentPrincipal() principal: AuthPrincipal,
    @Param('roomId') roomId: string,
  ) {
    return this.snapshots.latest(principal.userId, roomId);
  }

  @Post()
  @AgentOnly()
  create(
    @CurrentPrincipal() principal: AuthPrincipal,
    @Param('roomId') roomId: string,
    @Body(new ZodValidationPipe(createRoomSnapshotSchema))
    body: z.infer<typeof createRoomSnapshotSchema>,
  ) {
    return this.snapshots.create(
      principal.userId,
      requireAgentDevice(principal),
      roomId,
      body,
    );
  }
}
