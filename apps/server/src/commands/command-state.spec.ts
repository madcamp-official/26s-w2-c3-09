import { canTransition } from './command-state';
describe('command state machine', () => {
  it('allows the durable delivery path', () => {
    expect(canTransition('QUEUED', 'DELIVERED')).toBe(true);
    expect(canTransition('DELIVERED', 'ANALYZING')).toBe(true);
  });
  it('rejects skipped approval and terminal replay', () => {
    expect(canTransition('QUEUED', 'EXECUTING')).toBe(false);
    expect(canTransition('SUCCEEDED', 'EXECUTING')).toBe(false);
  });
});
