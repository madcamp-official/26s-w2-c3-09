import { commandUpdatedPayload } from './commands.service';

describe('commandUpdatedPayload', () => {
  it('keeps command.updated payload self-contained for targeted realtime reducers', () => {
    expect(
      commandUpdatedPayload({
        commandId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a1',
        roomId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a2',
        status: 'ANALYZING',
      }),
    ).toEqual({
      commandId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a1',
      roomId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a2',
      status: 'ANALYZING',
    });
  });
});
