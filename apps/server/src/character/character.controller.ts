import {
  Body,
  ConflictException,
  Controller,
  Get,
  Inject,
  Patch,
  UseGuards,
} from '@nestjs/common';
import { updateCharacterSchema } from '@mousekeeper/contracts';
import { characterProfiles, type Database } from '@mousekeeper/database';
import { eq } from 'drizzle-orm';
import { z } from 'zod';
import { CurrentPrincipal } from '../auth/auth-principal';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import { ZodValidationPipe } from '../common/zod-validation.pipe';
import { DATABASE } from '../database/database.module';
import {
  characterSelectionIsUnlocked,
  FIRST_CHARACTER_UNLOCK_AFFINITY,
  mergeCharacterAppearance,
  unlockedCharacterItems,
} from './character-policy';
@Controller('v1/character')
@UseGuards(FirebaseAuthGuard)
export class CharacterController {
  constructor(@Inject(DATABASE) private readonly db: Database) {}
  private async profile(userId: string) {
    await this.db
      .insert(characterProfiles)
      .values({ userId })
      .onConflictDoNothing();
    return (
      await this.db
        .select()
        .from(characterProfiles)
        .where(eq(characterProfiles.userId, userId))
        .limit(1)
    )[0];
  }
  @Get() async get(@CurrentPrincipal() p: AuthPrincipal) {
    const profile = await this.profile(p.userId);
    if (!profile) throw new Error('Character profile unavailable');
    const { userId: _, ...safeProfile } = profile;
    return {
      ...safeProfile,
      unlockedItems: unlockedCharacterItems(profile.affinityTotal),
      nextUnlockAffinity:
        profile.affinityTotal < FIRST_CHARACTER_UNLOCK_AFFINITY
          ? FIRST_CHARACTER_UNLOCK_AFFINITY
          : null,
      riveAssetStatus: 'UNCONFIGURED' as const,
    };
  }
  @Patch() async update(
    @CurrentPrincipal() p: AuthPrincipal,
    @Body(new ZodValidationPipe(updateCharacterSchema))
    body: z.infer<typeof updateCharacterSchema>,
  ) {
    const profile = await this.profile(p.userId);
    if (!profile) throw new Error('Character profile unavailable');
    const unlocked = unlockedCharacterItems(profile.affinityTotal);
    if (
      !characterSelectionIsUnlocked(unlocked, {
        furVariant: body.appearance?.furVariant,
        accessory: body.appearance?.accessory,
        roomTheme: body.roomTheme,
      })
    ) {
      throw new ConflictException({ code: 'FEATURE_LOCKED' });
    }
    const update = {
      ...(body.appearance
        ? {
            appearance: mergeCharacterAppearance(
              profile.appearance,
              body.appearance,
            ),
          }
        : {}),
      ...(Object.hasOwn(body, 'roomTheme')
        ? { roomTheme: body.roomTheme }
        : {}),
      updatedAt: new Date(),
    };
    const updated = (
      await this.db
        .update(characterProfiles)
        .set(update)
        .where(eq(characterProfiles.id, profile.id))
        .returning()
    )[0];
    const { userId: _, ...safeProfile } = updated;
    return { ...safeProfile, unlockedItems: unlocked };
  }
}
