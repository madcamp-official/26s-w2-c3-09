import { Injectable } from '@nestjs/common';
import {
  affinityEvents,
  characterProfiles,
  type Database,
} from '@mousekeeper/database';
import { eq, sql } from 'drizzle-orm';
type Transaction = Parameters<Parameters<Database['transaction']>[0]>[0];
@Injectable()
export class AffinityService {
  async append(
    tx: Transaction,
    input: {
      userId: string;
      eventType: string;
      delta: number;
      sourceDecisionId?: string;
      sourceExecutionId?: string;
    },
  ) {
    await tx
      .insert(characterProfiles)
      .values({ userId: input.userId })
      .onConflictDoNothing();
    const profile = (
      await tx
        .select()
        .from(characterProfiles)
        .where(eq(characterProfiles.userId, input.userId))
        .limit(1)
    )[0];
    if (!profile) throw new Error('Character profile unavailable');
    const event = (
      await tx
        .insert(affinityEvents)
        .values({
          characterProfileId: profile.id,
          eventType: input.eventType,
          delta: input.delta,
          sourceDecisionId: input.sourceDecisionId,
          sourceExecutionId: input.sourceExecutionId,
        })
        .onConflictDoNothing()
        .returning()
    )[0];
    if (event)
      await tx
        .update(characterProfiles)
        .set({
          affinityTotal: sql`${characterProfiles.affinityTotal} + ${input.delta}`,
          updatedAt: new Date(),
        })
        .where(eq(characterProfiles.id, profile.id));
    return event;
  }
}
