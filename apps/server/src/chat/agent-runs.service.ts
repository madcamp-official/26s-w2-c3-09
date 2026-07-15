import { createHash, randomUUID } from 'node:crypto';
import {
  ConflictException,
  Inject,
  Injectable,
  NotFoundException,
} from '@nestjs/common';
import {
  agentRunStatusSchema,
  agentToolNameSchema,
} from '@mousekeeper/contracts';
import { agentRuns, agentSteps, devices, rooms, type Database } from '@mousekeeper/database';
import { and, asc, eq, inArray, lte } from 'drizzle-orm';
import { canonicalJson } from '../common/canonical-json';
import { DATABASE } from '../database/database.module';
import Redis from 'ioredis';
import { REDIS } from '../presence/redis.module';

const RUN_TTL_MS = 10 * 60 * 1000;
const MAX_STEPS = 8;

@Injectable()
export class AgentRunsService {
  constructor(@Inject(DATABASE) private readonly db: Database, @Inject(REDIS) private readonly redis: Redis) {}

  async begin(input: {
    userId: string;
    roomId: string;
    sessionId: string;
    sourceMessageId: string;
  }) {
    const created = await this.db
      .insert(agentRuns)
      .values({ ...input, expiresAt: new Date(Date.now() + RUN_TTL_MS) })
      .onConflictDoNothing({ target: agentRuns.sourceMessageId })
      .returning();
    const run =
      created[0] ??
      (
        await this.db
          .select()
          .from(agentRuns)
          .where(eq(agentRuns.sourceMessageId, input.sourceMessageId))
          .limit(1)
      )[0];
    if (!run) throw new ConflictException({ code: 'AGENT_RUN_CONFLICT' });
    return this.publicRun(run);
  }

  async transition(
    userId: string,
    runId: string,
    status: string,
    options: {
      route?: string | null;
      failureCode?: string | null;
      resumeContext?: Record<string, unknown> | null;
    } = {},
  ) {
    const parsedStatus = agentRunStatusSchema.parse(status);
    const updated = (
      await this.db
        .update(agentRuns)
        .set({
          status: parsedStatus,
          route: options.route,
          failureCode: options.failureCode,
          resumeContext: options.resumeContext,
          updatedAt: new Date(),
        })
        .where(and(eq(agentRuns.id, runId), eq(agentRuns.userId, userId)))
        .returning()
    )[0];
    if (!updated) throw new NotFoundException({ code: 'AGENT_RUN_NOT_FOUND' });
    return this.publicRun(updated);
  }

  async addStep(
    userId: string,
    runId: string,
    toolName: string,
    input: Record<string, unknown>,
    externalRequestId?: string,
  ) {
    const tool = agentToolNameSchema.parse(toolName);
    const run = (
      await this.db
        .select()
        .from(agentRuns)
        .where(and(eq(agentRuns.id, runId), eq(agentRuns.userId, userId)))
        .limit(1)
    )[0];
    if (!run) throw new NotFoundException({ code: 'AGENT_RUN_NOT_FOUND' });
    if (run.currentStepCount >= MAX_STEPS) {
      throw new ConflictException({ code: 'AGENT_STEP_LIMIT' });
    }
    const inputHash = createHash('sha256')
      .update(canonicalJson(input))
      .digest('hex');
    const existing = await this.db
      .select()
      .from(agentSteps)
      .where(
        and(
          eq(agentSteps.runId, runId),
          eq(agentSteps.toolName, tool),
          eq(agentSteps.inputHash, inputHash),
        ),
      )
      .limit(1);
    if (existing[0]) return existing[0];

    return this.db.transaction(async (tx) => {
      const sequence = run.currentStepCount + 1;
      const step = (
        await tx
          .insert(agentSteps)
          .values({
            runId,
            sequence,
            toolName: tool,
            idempotencyKey: randomUUID(),
            inputHash,
            input,
            externalRequestId,
          })
          .returning()
      )[0]!;
      await tx
        .update(agentRuns)
        .set({ currentStepCount: sequence, updatedAt: new Date() })
        .where(eq(agentRuns.id, runId));
      return step;
    });
  }

  async get(userId: string, runId: string) {
    const run = (
      await this.db
        .select()
        .from(agentRuns)
        .where(and(eq(agentRuns.id, runId), eq(agentRuns.userId, userId)))
        .limit(1)
    )[0];
    if (!run) throw new NotFoundException({ code: 'AGENT_RUN_NOT_FOUND' });
    return this.publicRun(run);
  }

  async pendingTools(userId: string, deviceId: string) {
    const device = (
      await this.db
        .select()
        .from(devices)
        .where(and(eq(devices.id, deviceId), eq(devices.userId, userId), eq(devices.status, 'ACTIVE')))
        .limit(1)
    )[0];
    if (!device) throw new NotFoundException({ code: 'AGENT_DEVICE_NOT_FOUND' });
    return this.db
      .select({ step: agentSteps, run: agentRuns, room: rooms })
      .from(agentSteps)
      .innerJoin(agentRuns, eq(agentSteps.runId, agentRuns.id))
      .innerJoin(rooms, and(eq(agentRuns.roomId, rooms.id), eq(rooms.desktopDeviceId, deviceId)))
      .where(and(eq(agentSteps.status, 'QUEUED'), eq(agentRuns.status, 'WAITING_TOOL')))
      .limit(20);
  }

  async completeTool(
    userId: string,
    deviceId: string,
    stepId: string,
    status: 'SUCCEEDED' | 'FAILED',
    resultMetadata: Record<string, unknown> | null,
    failureCode: string | null,
  ) {
    return this.db.transaction(async (tx) => {
      const row = (
        await tx
          .select({ step: agentSteps, run: agentRuns, room: rooms })
          .from(agentSteps)
          .innerJoin(agentRuns, eq(agentSteps.runId, agentRuns.id))
          .innerJoin(rooms, and(eq(agentRuns.roomId, rooms.id), eq(rooms.desktopDeviceId, deviceId), eq(rooms.userId, userId)))
          .where(eq(agentSteps.id, stepId))
          .for('update')
          .limit(1)
      )[0];
      if (!row) throw new NotFoundException({ code: 'AGENT_TOOL_NOT_FOUND' });
      if (row.step.status === 'SUCCEEDED' || row.step.status === 'FAILED') return row.step;
      const safeMetadata = resultMetadata ? { ...resultMetadata } : null;
      if (row.step.toolName === 'DOCUMENT_EXTRACT' && safeMetadata && Array.isArray(safeMetadata.chunks)) {
        const chunks = safeMetadata.chunks;
        delete safeMetadata.chunks;
        if (this.redis.status === 'wait') await this.redis.connect();
        await this.redis.set(`agent:document:${row.step.id}`, JSON.stringify(chunks), 'EX', 600);
      }
      const updated = (
        await tx
          .update(agentSteps)
          .set({ status, resultMetadata: safeMetadata, failureCode, completedAt: new Date() })
          .where(eq(agentSteps.id, stepId))
          .returning()
      )[0]!;
      await tx
        .update(agentRuns)
        .set({ status: 'QUEUED', updatedAt: new Date() })
        .where(and(eq(agentRuns.id, row.run.id), eq(agentRuns.status, 'WAITING_TOOL')));
      return updated;
    });
  }

  async getEphemeralDocumentChunks(stepId: string): Promise<string[]> {
    if (this.redis.status === 'wait') await this.redis.connect();
    const raw = await this.redis.get(`agent:document:${stepId}`);
    if (!raw) return [];
    await this.redis.del(`agent:document:${stepId}`);
    try {
      const parsed: unknown = JSON.parse(raw);
      return Array.isArray(parsed) ? parsed.filter((value): value is string => typeof value === 'string').slice(0, 25) : [];
    } catch { return []; }
  }

  async answerClarification(
    userId: string,
    runId: string,
    selectedCandidate: string,
  ) {
    return this.db.transaction(async (tx) => {
      const run = (
        await tx
          .select()
          .from(agentRuns)
          .where(and(eq(agentRuns.id, runId), eq(agentRuns.userId, userId)))
          .for('update')
          .limit(1)
      )[0];
      if (!run) throw new NotFoundException({ code: 'AGENT_RUN_NOT_FOUND' });
      if (run.status !== 'WAITING_USER') {
        throw new ConflictException({ code: 'AGENT_RUN_NOT_WAITING_USER' });
      }
      const context = asRecord(run.resumeContext);
      const candidates = Array.isArray(context?.candidates)
        ? context.candidates.filter(
            (value): value is string => typeof value === 'string',
          )
        : [];
      if (!candidates.includes(selectedCandidate)) {
        throw new ConflictException({ code: 'INVALID_AGENT_CANDIDATE' });
      }
      const updated = (
        await tx
          .update(agentRuns)
          .set({
            status: 'QUEUED',
            resumeContext: { selectedCandidate, candidates },
            updatedAt: new Date(),
          })
          .where(eq(agentRuns.id, runId))
          .returning()
      )[0]!;
      return this.publicRun(updated);
    });
  }

  async claimNext() {
    return this.db.transaction(async (tx) => {
      const now = new Date();
      await tx
        .update(agentRuns)
        .set({ status: 'QUEUED', updatedAt: now })
        .where(
          and(
            eq(agentRuns.status, 'RUNNING'),
            lte(agentRuns.updatedAt, new Date(now.getTime() - 5 * 60_000)),
          ),
        );
      await tx
        .update(agentRuns)
        .set({ status: 'EXPIRED', updatedAt: now })
        .where(
          and(
            lte(agentRuns.expiresAt, now),
            inArray(agentRuns.status, ['QUEUED', 'RUNNING', 'WAITING_TOOL']),
          ),
        );
      const run = (
        await tx
          .select()
          .from(agentRuns)
          .where(eq(agentRuns.status, 'QUEUED'))
          .orderBy(asc(agentRuns.createdAt))
          .for('update', { skipLocked: true })
          .limit(1)
      )[0];
      if (!run) return null;
      const claimed = (
        await tx
          .update(agentRuns)
          .set({ status: 'RUNNING', updatedAt: now })
          .where(and(eq(agentRuns.id, run.id), eq(agentRuns.status, 'QUEUED')))
          .returning()
      )[0];
      return claimed ?? null;
    });
  }

  private publicRun(run: typeof agentRuns.$inferSelect) {
    const context = asRecord(run.resumeContext);
    const candidates = Array.isArray(context?.candidates)
      ? context.candidates.filter(
          (value): value is string => typeof value === 'string',
        )
      : [];
    return {
      id: run.id,
      status: run.status,
      route: run.route,
      sourceMessageId: run.sourceMessageId,
      currentStepCount: run.currentStepCount,
      expiresAt: run.expiresAt.toISOString(),
      ...(run.status === 'WAITING_USER' && candidates.length > 0
        ? { clarification: { candidates } }
        : {}),
    };
  }
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value != null && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}
