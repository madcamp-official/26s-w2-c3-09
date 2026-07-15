import { rooms, rules, type Database } from '@mousekeeper/database';
import { and, asc, eq } from 'drizzle-orm';
import type { RoomContext } from './ai.provider';

type Transaction = Parameters<Parameters<Database['transaction']>[0]>[0];
type DbExecutor = Database | Transaction;

export async function buildRoomContext(
  db: DbExecutor,
  roomId: string,
): Promise<RoomContext | null> {
  const room = (
    await db.select().from(rooms).where(eq(rooms.id, roomId)).limit(1)
  )[0];
  if (!room) return null;
  const roomRules = await db
    .select({ name: rules.name, definition: rules.definition })
    .from(rules)
    .where(and(eq(rules.roomId, roomId), eq(rules.enabled, true)))
    .orderBy(asc(rules.priority))
    .limit(20);
  return {
    roomName: room.name,
    rootAlias: room.rootAlias,
    existingRules: roomRules.map((rule) => ({
      name: rule.name,
      destinationTemplate: destinationTemplateFromDefinition(rule.definition),
    })),
  };
}

function destinationTemplateFromDefinition(value: unknown): string | null {
  if (value == null || typeof value !== 'object') return null;
  const action = (value as Record<string, unknown>).action;
  if (action == null || typeof action !== 'object') return null;
  const destination = (action as Record<string, unknown>).destinationTemplate;
  return typeof destination === 'string' ? destination : null;
}
