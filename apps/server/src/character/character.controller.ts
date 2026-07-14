import { Body, Controller, Get, Patch, UseGuards } from '@nestjs/common';
import { updateCharacterSchema } from '@mousekeeper/contracts';
import { z } from 'zod';
import { CurrentPrincipal } from '../auth/auth-principal';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import { ZodValidationPipe } from '../common/zod-validation.pipe';
import { CharacterService } from './character.service';
@Controller('v1/character')
@UseGuards(FirebaseAuthGuard)
export class CharacterController {
  constructor(private readonly character: CharacterService) {}
  @Get() get(@CurrentPrincipal() p: AuthPrincipal) {
    return this.character.get(p.userId);
  }
  @Patch() async update(
    @CurrentPrincipal() p: AuthPrincipal,
    @Body(new ZodValidationPipe(updateCharacterSchema))
    body: z.infer<typeof updateCharacterSchema>,
  ) {
    return this.character.update(p.userId, body);
  }
}
