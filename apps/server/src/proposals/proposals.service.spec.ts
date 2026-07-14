import { proposalCreatedPayload } from './proposals.service';

describe('proposalCreatedPayload', () => {
  it('keeps proposal.created payload self-contained for targeted realtime reducers', () => {
    expect(
      proposalCreatedPayload({
        proposalId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a1',
        roomId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a2',
        commandId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a3',
        status: 'OPEN',
        summary: { title: '정리 제안' },
        itemCount: 3,
        pendingProposalCount: 2,
      }),
    ).toEqual({
      proposalId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a1',
      roomId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a2',
      commandId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a3',
      status: 'OPEN',
      summary: { title: '정리 제안' },
      itemCount: 3,
      pendingProposalCount: 2,
    });
  });
});
