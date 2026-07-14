import {
  ConflictException,
  Inject,
  Injectable,
  InternalServerErrorException,
} from '@nestjs/common';
import {
  homeSummaryCharacterSchema,
  updateCharacterSchema,
} from '@mousekeeper/contracts';
import { characterProfiles, type Database } from '@mousekeeper/database';
import { eq } from 'drizzle-orm';
import { z } from 'zod';
import { DATABASE } from '../database/database.module';
import {
  characterSelectionIsUnlocked,
  FIRST_CHARACTER_UNLOCK_AFFINITY,
  mergeCharacterAppearance,
  unlockedCharacterItems,
} from './character-policy';

@Injectable()
export class CharacterService {
  constructor(@Inject(DATABASE) private readonly db: Database) {}

  private async profile(userId: string) {
    await this.db
      .insert(characterProfiles)
      .values({ userId })
      .onConflictDoNothing();
    const profile = (
      await this.db
        .select()
        .from(characterProfiles)
        .where(eq(characterProfiles.userId, userId))
        .limit(1)
    )[0];
    if (!profile) {
      throw new InternalServerErrorException({
        code: 'CHARACTER_PROFILE_UNAVAILABLE',
      });
    }
    return profile;
  }

  async get(userId: string) {
    const profile = await this.profile(userId);
    return homeSummaryCharacterSchema.parse({
      id: profile.id,
      appearance: profile.appearance,
      roomTheme: profile.roomTheme,
      affinityTotal: profile.affinityTotal,
      createdAt: profile.createdAt.toISOString(),
      updatedAt: profile.updatedAt.toISOString(),
      unlockedItems: unlockedCharacterItems(profile.affinityTotal),
      nextUnlockAffinity:
        profile.affinityTotal < FIRST_CHARACTER_UNLOCK_AFFINITY
          ? FIRST_CHARACTER_UNLOCK_AFFINITY
          : null,
      riveAssetStatus: 'UNCONFIGURED',
    });
  }

  async update(userId: string, body: z.infer<typeof updateCharacterSchema>) {
    const profile = await this.profile(userId);
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
    if (!updated) {
      throw new InternalServerErrorException({
        code: 'CHARACTER_PROFILE_UNAVAILABLE',
      });
    }
    return {
      id: updated.id,
      appearance: updated.appearance,
      roomTheme: updated.roomTheme,
      affinityTotal: updated.affinityTotal,
      createdAt: updated.createdAt,
      updatedAt: updated.updatedAt,
      unlockedItems: unlocked,
    };
  }
}
