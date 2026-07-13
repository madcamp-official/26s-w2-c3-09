jest.mock('../auth/auth.service', () => ({
  AuthService: class AuthService {},
}));

import { RealtimeDispatcher } from './realtime-dispatcher.service';

describe('RealtimeDispatcher publication serialization', () => {
  it('does not let an immediate publish and periodic flush claim the same row', async () => {
    let releaseImmediate!: (rows: never[]) => void;
    const immediate = new Promise<never[]>((resolve) => {
      releaseImmediate = resolve;
    });
    const immediateResult = {
      then: immediate.then.bind(immediate),
    };
    const flushResult = { limit: jest.fn().mockResolvedValue([]) };
    const orderBy = jest
      .fn()
      .mockReturnValueOnce(immediateResult)
      .mockReturnValueOnce(flushResult);
    const where = jest.fn().mockReturnValue({ orderBy });
    const from = jest.fn().mockReturnValue({ where });
    const select = jest.fn().mockReturnValue({ from });
    const database = { select };
    const gateway = {
      isReady: jest.fn().mockReturnValue(true),
      publish: jest.fn(),
    };
    const dispatcher = new RealtimeDispatcher(
      database as never,
      gateway as never,
      {} as never,
    );

    const publish = dispatcher.publishNow([
      '018f4c7b-1ad6-7c95-bf34-5e45881f98a1',
    ]);
    await Promise.resolve();
    expect(select).toHaveBeenCalledTimes(1);

    const flush = dispatcher.flush();
    await Promise.resolve();
    expect(select).toHaveBeenCalledTimes(1);

    releaseImmediate([]);
    await Promise.all([publish, flush]);
    expect(select).toHaveBeenCalledTimes(2);
    expect(gateway.publish).not.toHaveBeenCalled();
  });
});
