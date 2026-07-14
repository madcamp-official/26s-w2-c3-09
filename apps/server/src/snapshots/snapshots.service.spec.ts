import { roomSnapshotUpdatedPayload } from './snapshots.service';

describe('roomSnapshotUpdatedPayload', () => {
  it('keeps room.snapshot.updated payload self-contained for targeted realtime reducers', () => {
    expect(
      roomSnapshotUpdatedPayload({
        snapshotId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a1',
        roomId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a2',
        score: 88,
        metrics: {
          totalFileCount: 10,
          managedFileCount: 8,
          unorganizedFileCount: 2,
          deductions: [],
        },
        formulaVersion: 'mousekeeper-cleanliness-v1',
        calculatedAt: new Date('2026-07-13T00:00:00.000Z'),
      }),
    ).toEqual({
      snapshotId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a1',
      roomId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a2',
      score: 88,
      metrics: {
        totalFileCount: 10,
        managedFileCount: 8,
        unorganizedFileCount: 2,
        deductions: [],
      },
      formulaVersion: 'mousekeeper-cleanliness-v1',
      calculatedAt: '2026-07-13T00:00:00.000Z',
    });
  });
});
