import type { EventEnvelope } from '@housemouse/contracts';
import { toCharacterEvent } from './character-event';

const base: EventEnvelope = {
  eventId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a1',
  eventType: 'execution.updated',
  schemaVersion: 1,
  correlationId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a2',
  aggregateType: 'execution',
  aggregateId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a3',
  deviceId: null,
  roomId: null,
  sequence: 1,
  occurredAt: new Date().toISOString(),
  payload: {},
};

describe('toCharacterEvent', () => {
  it.each([
    ['EXECUTING', 'WORKING'],
    ['SUCCEEDED', 'SUCCESS'],
    ['FAILED', 'ERROR'],
    ['STALE', 'ERROR'],
  ])('maps execution %s to %s', (status, expected) => {
    expect(toCharacterEvent({ ...base, payload: { status } })?.kind).toBe(
      expected,
    );
  });

  it('does not invent a character state for unrelated events', () => {
    expect(toCharacterEvent({ ...base, eventType: 'rule.updated' })).toBeNull();
  });
});
