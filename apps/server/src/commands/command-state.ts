import { commandStatusSchema } from '@mousekeeper/contracts';
import { z } from 'zod';
type Status = z.infer<typeof commandStatusSchema>;

const transitions: Record<Status, readonly Status[]> = {
  QUEUED: ['DELIVERED'],
  DELIVERED: ['ANALYZING'],
  ANALYZING: ['PROPOSAL_READY', 'FAILED'],
  PROPOSAL_READY: ['WAITING_APPROVAL'],
  WAITING_APPROVAL: ['APPROVED', 'REJECTED', 'EXPIRED'],
  APPROVED: ['EXECUTING'],
  REJECTED: [],
  EXPIRED: [],
  EXECUTING: ['SUCCEEDED', 'PARTIALLY_SUCCEEDED', 'FAILED', 'STALE'],
  SUCCEEDED: [],
  PARTIALLY_SUCCEEDED: [],
  FAILED: [],
  STALE: [],
};
export function canTransition(from: Status, to: Status) {
  return transitions[from].includes(to);
}
