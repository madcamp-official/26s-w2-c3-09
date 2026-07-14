import { BadRequestException } from '@nestjs/common';
import { SmartCacheController } from './smart-cache.controller';

jest.mock('../auth/auth.service', () => ({
  AuthService: class AuthService {},
}));

jest.mock('../auth/firebase-auth.guard', () => ({
  FirebaseAuthGuard: class FirebaseAuthGuard {},
}));

describe('SmartCacheController', () => {
  const principal = {
    userId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a1',
    deviceId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a2',
    authProviderUid: 'desktop-device',
    displayName: 'Desktop',
    authType: 'DEVICE' as const,
  };

  it('keeps the planned smart-cache file list path as the mobile read endpoint', () => {
    const service = {
      list: jest.fn().mockReturnValue({
        files: [],
        pendingCommandWarning: false,
        desktopOnline: true,
      }),
    };
    const controller = new SmartCacheController(service as never);

    expect(
      controller.listSmartCacheFiles(
        principal,
        '018f4c7b-1ad6-7c95-bf34-5e45881f98a3',
      ),
    ).toEqual({
      files: [],
      pendingCommandWarning: false,
      desktopOnline: true,
    });
    expect(service.list).toHaveBeenCalledWith(
      principal.userId,
      '018f4c7b-1ad6-7c95-bf34-5e45881f98a3',
    );
  });

  it('delegates desktop source-change stale reports with an idempotency key', () => {
    const service = {
      markStale: jest.fn().mockReturnValue({
        roomId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a3',
        sourceRelativePath: 'docs/report.pdf',
        reason: 'SOURCE_CHANGED',
        staleCount: 1,
      }),
    };
    const controller = new SmartCacheController(service as never);
    const body = {
      roomId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a3',
      sourceRelativePath: 'docs/report.pdf',
      reason: 'SOURCE_CHANGED' as const,
    };

    expect(controller.markStale(principal, 'stale-report-1', body)).toEqual({
      ...body,
      staleCount: 1,
    });
    expect(service.markStale).toHaveBeenCalledWith(
      principal.userId,
      principal.deviceId,
      body,
    );
  });

  it('rejects stale reports without an idempotency key', () => {
    const controller = new SmartCacheController({
      markStale: jest.fn(),
    } as never);

    expect(() =>
      controller.markStale(principal, undefined, {
        roomId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a3',
        sourceRelativePath: null,
        reason: 'REINDEXED',
      }),
    ).toThrow(BadRequestException);
  });

  it('delegates verified mobile cached-file access events', () => {
    const mobilePrincipal = {
      ...principal,
      deviceId: null,
      authProviderUid: 'firebase-user',
      displayName: 'Mobile',
      authType: 'FIREBASE' as const,
    };
    const service = {
      recordAccess: jest.fn().mockReturnValue({
        cachedFileId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a4',
        eventType: 'DOWNLOAD_COMPLETED',
        usageScore: 15,
        lastAccessedAt: '2026-07-14T01:02:03.000Z',
      }),
    };
    const controller = new SmartCacheController(service as never);
    const body = { eventType: 'DOWNLOAD_COMPLETED' as const };

    expect(
      controller.recordAccess(
        mobilePrincipal,
        '018f4c7b-1ad6-7c95-bf34-5e45881f98a4',
        body,
      ),
    ).toEqual({
      cachedFileId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a4',
      eventType: 'DOWNLOAD_COMPLETED',
      usageScore: 15,
      lastAccessedAt: '2026-07-14T01:02:03.000Z',
    });
    expect(service.recordAccess).toHaveBeenCalledWith(
      mobilePrincipal.userId,
      '018f4c7b-1ad6-7c95-bf34-5e45881f98a4',
      body,
    );
  });
});
