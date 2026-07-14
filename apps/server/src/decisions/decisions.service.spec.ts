import { decisionCreatedPayload } from './decisions.service';

describe('decisionCreatedPayload', () => {
  it('keeps decision.created payload self-contained for targeted realtime reducers', () => {
    expect(
      decisionCreatedPayload({
        decisionId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a1',
        proposalId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a2',
        roomId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a3',
        commandId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a4',
        decisionType: 'APPROVE',
        proposalStatus: 'APPROVED',
        commandStatus: 'APPROVED',
        pendingProposalCount: 1,
      }),
    ).toEqual({
      decisionId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a1',
      proposalId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a2',
      roomId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a3',
      commandId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a4',
      decisionType: 'APPROVE',
      proposalStatus: 'APPROVED',
      commandStatus: 'APPROVED',
      pendingProposalCount: 1,
    });
  });
});
