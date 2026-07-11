import type { EventEnvelope } from '@housemouse/contracts';

type CharacterKind =
  'ANALYZING' | 'WAITING_APPROVAL' | 'WORKING' | 'SUCCESS' | 'ERROR';

export function toCharacterEvent(event: EventEnvelope) {
  const kind = characterKind(event);
  if (!kind) return null;
  return {
    eventId: event.eventId,
    roomId: event.roomId,
    kind,
    occurredAt: event.occurredAt,
  };
}

function characterKind(event: EventEnvelope): CharacterKind | null {
  if (event.eventType === 'proposal.created') return 'WAITING_APPROVAL';
  if (event.eventType === 'command.updated') {
    if (event.payload.status === 'ANALYZING') return 'ANALYZING';
    if (event.payload.status === 'FAILED') return 'ERROR';
  }
  if (event.eventType === 'execution.updated') {
    const status = event.payload.status;
    if (status === 'EXECUTING') return 'WORKING';
    if (status === 'SUCCEEDED') return 'SUCCESS';
    if (
      status === 'PARTIALLY_SUCCEEDED' ||
      status === 'FAILED' ||
      status === 'STALE'
    )
      return 'ERROR';
  }
  return null;
}
