jest.mock('../auth/firebase-auth.guard', () => ({
  FirebaseAuthGuard: class FirebaseAuthGuard {},
}));

import { RulesController } from './rules.controller';

describe('RulesController', () => {
  it('delegates rule draft preview to the service with the authenticated user', async () => {
    const previewDraft = jest.fn().mockResolvedValue({
      status: 'READY',
      draft: {},
      items: [],
      truncated: false,
    });
    const controller = new RulesController({ previewDraft } as never);

    await expect(
      controller.previewDraft(
        { userId: 'user-1', authType: 'FIREBASE' } as never,
        'draft-1',
        {},
      ),
    ).resolves.toMatchObject({ items: [], truncated: false });
    expect(previewDraft).toHaveBeenCalledWith('user-1', 'draft-1');
  });
});
