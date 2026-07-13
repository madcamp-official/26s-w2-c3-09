import {
  ConflictException,
  ForbiddenException,
  Inject,
  Injectable,
  NotFoundException,
} from '@nestjs/common';
import { createProposalSchema } from '@mousekeeper/contracts';
import {
  auditEvents,
  commands,
  devices,
  proposalItems,
  proposals,
  rooms,
  type Database,
} from '@mousekeeper/database';
import { and, eq } from 'drizzle-orm';
import { z } from 'zod';
import { DATABASE } from '../database/database.module';
import { SyncService } from '../sync/sync.service';

@Injectable()
export class ProposalsService {
  constructor(
    @Inject(DATABASE) private readonly db: Database,
    private readonly sync: SyncService,
  ) {}
  async create(
    userId: string,
    deviceId: string,
    idempotencyKey: string,
    body: z.infer<typeof createProposalSchema>,
  ) {
    return this.db.transaction(async (tx) => {
      const command = (
        await tx
          .select({ command: commands, device: devices })
          .from(commands)
          .innerJoin(devices, eq(commands.targetDeviceId, devices.id))
          .where(
            and(
              eq(commands.id, body.commandId),
              eq(commands.roomId, body.roomId),
              eq(devices.userId, userId),
              eq(commands.targetDeviceId, deviceId),
            ),
          )
          .limit(1)
      )[0];
      if (!command) throw new ForbiddenException({ code: 'FORBIDDEN' });
      const replay = (
        await tx
          .select()
          .from(proposals)
          .where(eq(proposals.idempotencyKey, idempotencyKey))
          .limit(1)
      )[0];
      if (replay) {
        if (
          replay.commandId !== body.commandId ||
          replay.roomId !== body.roomId
        )
          throw new ConflictException({ code: 'IDEMPOTENCY_CONFLICT' });
        const replayItems = await tx
          .select()
          .from(proposalItems)
          .where(eq(proposalItems.proposalId, replay.id));
        return { ...this.publicProposal(replay), items: replayItems };
      }
      if (command.command.status !== 'ANALYZING')
        throw new ConflictException({ code: 'INVALID_STATE_TRANSITION' });
      const proposal = (
        await tx
          .insert(proposals)
          .values({
            commandId: body.commandId,
            roomId: body.roomId,
            summary: body.summary,
            idempotencyKey,
            expiresAt: body.expiresAt ? new Date(body.expiresAt) : null,
          })
          .onConflictDoNothing()
          .returning()
      )[0];
      if (!proposal) {
        const existing = (
          await tx
            .select()
            .from(proposals)
            .where(eq(proposals.idempotencyKey, idempotencyKey))
            .limit(1)
        )[0];
        if (
          !existing ||
          existing.commandId !== body.commandId ||
          existing.roomId !== body.roomId
        )
          throw new ConflictException({ code: 'IDEMPOTENCY_CONFLICT' });
        const existingItems = await tx
          .select()
          .from(proposalItems)
          .where(eq(proposalItems.proposalId, existing.id));
        return { ...this.publicProposal(existing), items: existingItems };
      }
      const items = await tx
        .insert(proposalItems)
        .values(
          body.items.map((item) => ({ ...item, proposalId: proposal.id })),
        )
        .returning();
      await tx
        .update(commands)
        .set({ status: 'WAITING_APPROVAL' })
        .where(eq(commands.id, body.commandId));
      await this.sync.append(tx, {
        userId,
        deviceId: command.command.targetDeviceId,
        roomId: body.roomId,
        eventType: 'proposal.created',
        aggregateType: 'proposal',
        aggregateId: proposal.id,
        payload: { commandId: body.commandId },
      });
      await tx.insert(auditEvents).values({
        userId,
        deviceId,
        roomId: body.roomId,
        eventType: 'proposal.created',
        aggregateType: 'proposal',
        aggregateId: proposal.id,
        metadata: { itemCount: items.length },
      });
      return { ...this.publicProposal(proposal), items };
    });
  }
  async get(userId: string, proposalId: string) {
    const proposal = (
      await this.db
        .select({ proposal: proposals })
        .from(proposals)
        .innerJoin(
          rooms,
          and(eq(proposals.roomId, rooms.id), eq(rooms.userId, userId)),
        )
        .where(eq(proposals.id, proposalId))
        .limit(1)
    )[0]?.proposal;
    if (!proposal) throw new NotFoundException({ code: 'NOT_FOUND' });
    const items = await this.db
      .select()
      .from(proposalItems)
      .where(eq(proposalItems.proposalId, proposalId));
    return { ...this.publicProposal(proposal), items };
  }

  async openForRoom(userId: string, roomId: string) {
    const room = (
      await this.db
        .select()
        .from(rooms)
        .where(and(eq(rooms.id, roomId), eq(rooms.userId, userId)))
        .limit(1)
    )[0];
    if (!room) throw new NotFoundException({ code: 'NOT_FOUND' });
    const open = await this.db
      .select()
      .from(proposals)
      .where(and(eq(proposals.roomId, roomId), eq(proposals.status, 'OPEN')));
    return open.map((proposal) => this.publicProposal(proposal));
  }

  private publicProposal(proposal: typeof proposals.$inferSelect) {
    const { idempotencyKey: _, ...safe } = proposal;
    return safe;
  }
}
