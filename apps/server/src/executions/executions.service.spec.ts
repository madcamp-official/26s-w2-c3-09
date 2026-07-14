import { executionUpdatedPayload } from './executions.service';

describe('executionUpdatedPayload', () => {
  it('keeps execution.updated payload self-contained for targeted realtime reducers', () => {
    expect(
      executionUpdatedPayload({
        executionId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a1',
        roomId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a2',
        status: 'SUCCEEDED',
      }),
    ).toEqual({
      executionId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a1',
      roomId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a2',
      status: 'SUCCEEDED',
    });
  });
});
