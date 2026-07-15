import {
  ConflictException,
  ForbiddenException,
  Inject,
  Injectable,
  NotFoundException,
} from '@nestjs/common';
import {
  createExecutionSchema,
  updateExecutionSchema,
} from '@mousekeeper/contracts';
import {
  auditEvents,
  commands,
  decisions,
  devices,
  executions,
  proposals,
  rooms,
  type Database,
} from '@mousekeeper/database';
import { and, desc, eq } from 'drizzle-orm';
import { z } from 'zod';
import { isDeepStrictEqual } from 'node:util';
import { DATABASE } from '../database/database.module';
import { SyncService } from '../sync/sync.service';
import { AffinityService } from '../affinity/affinity.service';
import { ChatService } from '../chat/chat.service';

@Injectable()
export class ExecutionsService {
  constructor(
    @Inject(DATABASE) private readonly db: Database,
    private readonly sync: SyncService,
    private readonly affinity: AffinityService,
    private readonly chat: ChatService,
  ) {}
  async create(
    userId: string,
    key: string,
    body: z.infer<typeof createExecutionSchema>,
  ) {
    return this.db.transaction(async (tx) => {
      const source = (
        await tx
          .select({ proposal: proposals, decision: decisions, device: devices })
          .from(proposals)
          .innerJoin(decisions, eq(decisions.proposalId, proposals.id))
          .innerJoin(commands, eq(commands.id, proposals.commandId))
          .innerJoin(devices, eq(devices.id, body.desktopDeviceId))
          .where(
            and(
              eq(proposals.id, body.proposalId),
              eq(decisions.id, body.decisionId),
              eq(decisions.decisionType, 'APPROVE'),
              eq(commands.targetDeviceId, body.desktopDeviceId),
              eq(devices.userId, userId),
              eq(devices.status, 'ACTIVE'),
            ),
          )
          .for('update')
          .limit(1)
      )[0];
      if (!source) throw new ForbiddenException({ code: 'FORBIDDEN' });
      if (source.proposal.status !== 'APPROVED')
        throw new ConflictException({ code: 'INVALID_STATE_TRANSITION' });
      const inserted = (
        await tx
          .insert(executions)
          .values({ ...body, idempotencyKey: key })
          .onConflictDoNothing()
          .returning()
      )[0];
      if (!inserted) {
        const existing = (
          await tx
            .select()
            .from(executions)
            .where(eq(executions.idempotencyKey, key))
            .limit(1)
        )[0];
        if (!existing) throw new ConflictException({ code: 'CONFLICT' });
        return this.publicExecution(existing);
      }
      const execution = inserted;
      await tx
        .update(commands)
        .set({ status: 'EXECUTING' })
        .where(eq(commands.id, source.proposal.commandId));
      await this.sync.append(tx, {
        userId,
        deviceId: body.desktopDeviceId,
        roomId: source.proposal.roomId,
        eventType: 'execution.updated',
        aggregateType: 'execution',
        aggregateId: execution.id,
        payload: executionUpdatedPayload({
          executionId: execution.id,
          roomId: source.proposal.roomId,
          status: execution.status,
        }),
      });
      return this.publicExecution(execution);
    });
  }
  async update(
    userId: string,
    deviceId: string,
    executionId: string,
    idempotencyKey: string,
    body: z.infer<typeof updateExecutionSchema>,
  ) {
    return this.db.transaction(async (tx) => {
      const replay = (
        await tx
          .select({ execution: executions, device: devices })
          .from(executions)
          .innerJoin(devices, eq(executions.desktopDeviceId, devices.id))
          .where(
            and(
              eq(executions.resultIdempotencyKey, idempotencyKey),
              eq(devices.userId, userId),
              eq(executions.desktopDeviceId, deviceId),
            ),
          )
          .limit(1)
      )[0]?.execution;
      if (replay) {
        if (
          replay.id !== executionId ||
          replay.status !== body.status ||
          !isDeepStrictEqual(replay.resultSummary, body.resultSummary)
        )
          throw new ConflictException({ code: 'IDEMPOTENCY_CONFLICT' });
        return this.publicExecution(replay);
      }
      const source = (
        await tx
          .select({
            execution: executions,
            proposal: proposals,
            device: devices,
          })
          .from(executions)
          .innerJoin(proposals, eq(executions.proposalId, proposals.id))
          .innerJoin(devices, eq(executions.desktopDeviceId, devices.id))
          .where(
            and(
              eq(executions.id, executionId),
              eq(executions.desktopDeviceId, deviceId),
              eq(devices.userId, userId),
              eq(devices.status, 'ACTIVE'),
            ),
          )
          .for('update')
          .limit(1)
      )[0];
      if (!source) throw new NotFoundException({ code: 'NOT_FOUND' });
      if (source.execution.resultIdempotencyKey === idempotencyKey) {
        if (
          source.execution.status !== body.status ||
          !isDeepStrictEqual(source.execution.resultSummary, body.resultSummary)
        )
          throw new ConflictException({ code: 'IDEMPOTENCY_CONFLICT' });
        return this.publicExecution(source.execution);
      }
      if (source.execution.status !== 'EXECUTING')
        throw new ConflictException({ code: 'INVALID_STATE_TRANSITION' });
      const updated = (
        await tx
          .update(executions)
          .set({
            status: body.status,
            resultSummary: body.resultSummary,
            resultIdempotencyKey: idempotencyKey,
            finishedAt: new Date(),
          })
          .where(eq(executions.id, executionId))
          .returning()
      )[0];
      if (!updated) throw new ConflictException({ code: 'CONFLICT' });
      const finalCommandStatus =
        body.status === 'ROLLED_BACK' ? 'FAILED' : body.status;
      await tx
        .update(commands)
        .set({ status: finalCommandStatus, finishedAt: new Date() })
        .where(eq(commands.id, source.proposal.commandId));
      await this.sync.append(tx, {
        userId,
        deviceId: source.execution.desktopDeviceId,
        roomId: source.proposal.roomId,
        eventType: 'execution.updated',
        aggregateType: 'execution',
        aggregateId: executionId,
        payload: executionUpdatedPayload({
          executionId,
          roomId: source.proposal.roomId,
          status: body.status,
        }),
      });
      await tx.insert(auditEvents).values({
        userId,
        deviceId: source.execution.desktopDeviceId,
        roomId: source.proposal.roomId,
        eventType: 'execution.completed',
        aggregateType: 'execution',
        aggregateId: executionId,
        metadata: { status: body.status },
      });
      if (body.status === 'SUCCEEDED') {
        await this.affinity.append(tx, {
          userId,
          eventType: 'EXECUTION_SUCCEEDED',
          delta: 2,
          sourceExecutionId: executionId,
        });
      }
      if (source.proposal.sessionId) {
        await this.chat.postSystemMessageIn(tx, {
          userId,
          roomId: source.proposal.roomId,
          sessionId: source.proposal.sessionId,
          messageType: 'EXECUTION_RESULT',
          content: executionResultChatContent(updated),
          structuredPayload: executionResultChatPayload(updated),
        });
      }
      return this.publicExecution(updated);
    });
  }

  async listForRoom(userId: string, roomId: string) {
    const room = (
      await this.db
        .select({ id: rooms.id })
        .from(rooms)
        .where(and(eq(rooms.id, roomId), eq(rooms.userId, userId)))
        .limit(1)
    )[0];
    if (!room) throw new NotFoundException({ code: 'NOT_FOUND' });
    const history = await this.db
      .select({ execution: executions, proposal: proposals })
      .from(executions)
      .innerJoin(proposals, eq(executions.proposalId, proposals.id))
      .where(eq(proposals.roomId, roomId))
      .orderBy(desc(executions.startedAt));
    return history.map((item) => ({
      execution: this.publicExecution(item.execution),
      proposal: this.publicProposal(item.proposal),
    }));
  }

  async get(userId: string, executionId: string) {
    const result = (
      await this.db
        .select({ execution: executions, proposal: proposals })
        .from(executions)
        .innerJoin(proposals, eq(executions.proposalId, proposals.id))
        .innerJoin(
          rooms,
          and(eq(proposals.roomId, rooms.id), eq(rooms.userId, userId)),
        )
        .where(eq(executions.id, executionId))
        .limit(1)
    )[0];
    if (!result) throw new NotFoundException({ code: 'NOT_FOUND' });
    return {
      execution: this.publicExecution(result.execution),
      proposal: this.publicProposal(result.proposal),
    };
  }

  private publicExecution(execution: typeof executions.$inferSelect) {
    const { idempotencyKey: _, resultIdempotencyKey: __, ...safe } = execution;
    return safe;
  }

  private publicProposal(proposal: typeof proposals.$inferSelect) {
    const { idempotencyKey: _, ...safe } = proposal;
    return safe;
  }
}

export function executionUpdatedPayload(input: {
  executionId: string;
  roomId: string;
  status: string;
}) {
  return {
    executionId: input.executionId,
    roomId: input.roomId,
    status: input.status,
  };
}

function asNonNegativeInt(value: unknown): number {
  return typeof value === 'number' && Number.isInteger(value) && value >= 0
    ? value
    : 0;
}

/**
 * Desktop serializes `file_engine_cli::execute::ExecuteReport` as
 * `{executed_count, skipped_count, rejected_count, results: [...]}` into the
 * untyped `resultSummary` field. Best-effort: unknown shapes fall back to
 * zero counts rather than throwing.
 */
function extractResultCounts(resultSummary: unknown) {
  const summary =
    resultSummary &&
    typeof resultSummary === 'object' &&
    !Array.isArray(resultSummary)
      ? (resultSummary as Record<string, unknown>)
      : {};
  return {
    executedCount: asNonNegativeInt(
      summary.executed_count ?? summary.executedCount,
    ),
    skippedCount: asNonNegativeInt(
      summary.skipped_count ?? summary.skippedCount,
    ),
    rejectedCount: asNonNegativeInt(
      summary.rejected_count ?? summary.rejectedCount,
    ),
    items: Array.isArray(summary.results) ? summary.results : [],
  };
}

export function executionResultChatPayload(
  execution: Pick<
    typeof executions.$inferSelect,
    'id' | 'proposalId' | 'status' | 'resultSummary'
  >,
): Record<string, unknown> {
  return {
    id: execution.id,
    executionId: execution.id,
    proposalId: execution.proposalId,
    status: execution.status,
    ...extractResultCounts(execution.resultSummary),
  };
}

export function executionResultChatContent(
  execution: Pick<typeof executions.$inferSelect, 'status' | 'resultSummary'>,
): string {
  const counts = extractResultCounts(execution.resultSummary);
  return `실행 결과: ${execution.status} (완료 ${counts.executedCount}건, 건너뜀 ${counts.skippedCount}건, 거부 ${counts.rejectedCount}건)`;
}
