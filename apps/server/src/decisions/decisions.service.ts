import {
  BadRequestException,
  ConflictException,
  Inject,
  Injectable,
  NotFoundException,
} from '@nestjs/common';
import { createDecisionSchema } from '@mousekeeper/contracts';
import {
  auditEvents,
  commands,
  decisions,
  devices,
  executions,
  proposalItems,
  proposals,
  rooms,
  type Database,
} from '@mousekeeper/database';
import { and, count, eq, isNull } from 'drizzle-orm';
import { z } from 'zod';
import { DATABASE } from '../database/database.module';
import { SyncService } from '../sync/sync.service';
import { AffinityService } from '../affinity/affinity.service';
import { canonicalJson } from '../common/canonical-json';
@Injectable()
export class DecisionsService {
  constructor(
    @Inject(DATABASE) private readonly db: Database,
    private readonly sync: SyncService,
    private readonly affinity: AffinityService,
  ) {}
  async create(
    userId: string,
    proposalId: string,
    key: string,
    body: z.infer<typeof createDecisionSchema>,
  ) {
    return this.db.transaction(async (tx) => {
      const initialReplay = (
        await tx
          .select()
          .from(decisions)
          .where(
            and(
              eq(decisions.userId, userId),
              eq(decisions.idempotencyKey, key),
            ),
          )
          .limit(1)
      )[0];
      if (initialReplay)
        return this.validateReplay(initialReplay, proposalId, body);
      const owned = (
        await tx
          .select({ proposal: proposals, room: rooms })
          .from(proposals)
          .innerJoin(rooms, eq(proposals.roomId, rooms.id))
          .where(and(eq(proposals.id, proposalId), eq(rooms.userId, userId)))
          .for('update')
          .limit(1)
      )[0];
      if (!owned) throw new NotFoundException({ code: 'NOT_FOUND' });
      if (
        owned.proposal.status !== 'OPEN' ||
        (owned.proposal.expiresAt && owned.proposal.expiresAt <= new Date())
      ) {
        const concurrentReplay = (
          await tx
            .select()
            .from(decisions)
            .where(
              and(
                eq(decisions.userId, userId),
                eq(decisions.idempotencyKey, key),
              ),
            )
            .limit(1)
        )[0];
        if (concurrentReplay)
          return this.validateReplay(concurrentReplay, proposalId, body);
        throw new ConflictException({ code: 'INVALID_STATE_TRANSITION' });
      }
      const items = await tx
        .select({ id: proposalItems.id })
        .from(proposalItems)
        .where(eq(proposalItems.proposalId, proposalId));
      if (
        body.decisionType === 'APPROVE' &&
        (body.approvedItemIds.length !== items.length ||
          items.some((item) => !body.approvedItemIds.includes(item.id)))
      )
        throw new BadRequestException({
          code: 'VALIDATION_FAILED',
          message: 'MVP approval must include every proposal item',
        });
      const inserted = (
        await tx
          .insert(decisions)
          .values({
            proposalId,
            userId,
            decisionType: body.decisionType,
            approvedItemIds: body.approvedItemIds,
            idempotencyKey: key,
          })
          .onConflictDoNothing()
          .returning()
      )[0];
      if (!inserted) {
        const existing = (
          await tx
            .select()
            .from(decisions)
            .where(
              and(
                eq(decisions.userId, userId),
                eq(decisions.idempotencyKey, key),
              ),
            )
            .limit(1)
        )[0];
        if (!existing) throw new ConflictException({ code: 'CONFLICT' });
        return this.validateReplay(existing, proposalId, body);
      }
      const decision = inserted;
      const approved = body.decisionType === 'APPROVE';
      const finalProposalStatus = approved ? 'APPROVED' : 'REJECTED';
      const finalCommandStatus = approved ? 'APPROVED' : 'REJECTED';
      await tx
        .update(proposals)
        .set({ status: finalProposalStatus })
        .where(eq(proposals.id, proposalId));
      await tx
        .update(commands)
        .set({
          status: finalCommandStatus,
          ...(!approved ? { finishedAt: new Date() } : {}),
        })
        .where(eq(commands.id, owned.proposal.commandId));
      const pendingProposalCount = Number(
        (
          await tx
            .select({ value: count(proposals.id) })
            .from(proposals)
            .where(
              and(
                eq(proposals.roomId, owned.room.id),
                eq(proposals.status, 'OPEN'),
              ),
            )
        )[0]?.value ?? 0,
      );
      await this.sync.append(tx, {
        userId,
        deviceId: owned.room.desktopDeviceId,
        roomId: owned.room.id,
        eventType: 'decision.created',
        aggregateType: 'decision',
        aggregateId: decision.id,
        payload: decisionCreatedPayload({
          decisionId: decision.id,
          proposalId,
          roomId: owned.room.id,
          commandId: owned.proposal.commandId,
          decisionType: decision.decisionType,
          proposalStatus: finalProposalStatus,
          commandStatus: finalCommandStatus,
          pendingProposalCount,
        }),
      });
      await tx.insert(auditEvents).values({
        userId,
        deviceId: owned.room.desktopDeviceId,
        roomId: owned.room.id,
        eventType: 'decision.created',
        aggregateType: 'decision',
        aggregateId: decision.id,
        metadata: { decisionType: decision.decisionType },
      });
      if (approved) {
        await this.affinity.append(tx, {
          userId,
          eventType: 'PROPOSAL_APPROVED',
          delta: 1,
          sourceDecisionId: decision.id,
        });
      }
      return this.publicDecision(decision);
    });
  }

  async pending(userId: string, deviceId: string) {
    const device = (
      await this.db
        .select({ id: devices.id })
        .from(devices)
        .where(
          and(
            eq(devices.id, deviceId),
            eq(devices.userId, userId),
            eq(devices.status, 'ACTIVE'),
          ),
        )
        .limit(1)
    )[0];
    if (!device) throw new NotFoundException({ code: 'NOT_FOUND' });

    const rows = await this.db
      .select({ decision: decisions, proposal: proposals })
      .from(decisions)
      .innerJoin(proposals, eq(decisions.proposalId, proposals.id))
      .innerJoin(commands, eq(proposals.commandId, commands.id))
      .leftJoin(executions, eq(executions.decisionId, decisions.id))
      .where(
        and(
          eq(decisions.userId, userId),
          eq(decisions.decisionType, 'APPROVE'),
          eq(commands.targetDeviceId, deviceId),
          isNull(executions.id),
        ),
      );
    return Promise.all(
      rows.map(async (row) => ({
        decision: this.publicDecision(row.decision),
        proposal: this.publicProposal(row.proposal),
        items: await this.db
          .select()
          .from(proposalItems)
          .where(eq(proposalItems.proposalId, row.proposal.id)),
      })),
    );
  }

  private publicDecision(decision: typeof decisions.$inferSelect) {
    const { userId: _, idempotencyKey: __, ...safe } = decision;
    return safe;
  }

  private validateReplay(
    decision: typeof decisions.$inferSelect,
    proposalId: string,
    body: z.infer<typeof createDecisionSchema>,
  ) {
    if (
      decision.proposalId !== proposalId ||
      decision.decisionType !== body.decisionType ||
      canonicalJson(decision.approvedItemIds) !==
        canonicalJson(body.approvedItemIds)
    )
      throw new ConflictException({ code: 'IDEMPOTENCY_CONFLICT' });
    return this.publicDecision(decision);
  }

  private publicProposal(proposal: typeof proposals.$inferSelect) {
    const { idempotencyKey: _, ...safe } = proposal;
    return safe;
  }
}

export function decisionCreatedPayload(input: {
  decisionId: string;
  proposalId: string;
  roomId: string;
  commandId: string;
  decisionType: string;
  proposalStatus: string;
  commandStatus: string;
  pendingProposalCount: number;
}) {
  return {
    decisionId: input.decisionId,
    proposalId: input.proposalId,
    roomId: input.roomId,
    commandId: input.commandId,
    decisionType: input.decisionType,
    proposalStatus: input.proposalStatus,
    commandStatus: input.commandStatus,
    pendingProposalCount: input.pendingProposalCount,
  };
}
