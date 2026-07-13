import {
  Body,
  Controller,
  Get,
  Param,
  Post,
  Query,
  UseGuards,
} from '@nestjs/common';
import {
  claimPairingSessionSchema,
  createPairingSessionSchema,
} from '@mousekeeper/contracts';
import { z } from 'zod';
import { CurrentPrincipal } from '../auth/auth-principal';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import { ZodValidationPipe } from '../common/zod-validation.pipe';
import { PairingService } from './pairing.service';

@Controller('v1/pairing-sessions')
export class PairingController {
  constructor(private readonly pairing: PairingService) {}
  @Post()
  create(
    @Body(new ZodValidationPipe(createPairingSessionSchema))
    body: z.infer<typeof createPairingSessionSchema>,
  ) {
    return this.pairing.create(body);
  }
  @Post('claim')
  @UseGuards(FirebaseAuthGuard)
  claim(
    @CurrentPrincipal() p: AuthPrincipal,
    @Body(new ZodValidationPipe(claimPairingSessionSchema))
    body: z.infer<typeof claimPairingSessionSchema>,
  ) {
    return this.pairing.claim(p.userId, body.code);
  }
  @Get(':id/status')
  status(@Param('id') id: string, @Query('nonce') nonce: string) {
    return this.pairing.status(id, nonce);
  }
}
