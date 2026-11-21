/**
 * Unit tests for creating a profile on first sign-in: Supabase keeps the sign-in in its own database, so the trigger
 * that used to add a profile row can no longer reach this one, and the server adds it the first time a signed-in
 * person makes a request. An existing profile is never changed, and a failed write never refuses the person.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';
import { fakeRequest, runMiddleware } from '../../support/auth.ts';
import { tokenWithAal } from '../../support/mfa.ts';

ownDataDir('oya-profile-on-sign-in-');
const { ensureProfile, findProfile } = await import('../../../../src/modules/auth/repository.ts');
const { userAuthMiddleware, sessionUsers } = await import('../../../../src/modules/auth/middleware.ts');
const { getConnection } = await import('../../../../src/platform/storage/index.ts');

const real = { ...sessionUsers };
afterEach(() => {
  Object.assign(sessionUsers, real);
  mock.restoreAll();
});

/** A signed-in user with a unique id, so each test starts with no profile. */
let n = 0;
const someone = (extra: Record<string, unknown> = {}) => ({
  id: `u-${Date.now()}-${n++}`,
  email: 'ada@example.com',
  ...extra,
});

describe('ensureProfile', () => {
  it('creates a missing profile with the email and the name before @', async () => {
    const user = someone();
    await ensureProfile(user);
    const profile = await findProfile(user.id);
    assert.deepEqual([profile.email, profile.display_name], ['ada@example.com', 'ada']);
  });

  it('takes the display name chosen at sign-up when there is one', async () => {
    const user = someone({ user_metadata: { display_name: 'Ada Lovelace' } });
    await ensureProfile(user);
    assert.equal((await findProfile(user.id)).display_name, 'Ada Lovelace');
  });

  it('leaves an existing profile, its role included, as it is', async () => {
    const user = someone();
    await getConnection().upsert('profiles', [
      { id: user.id, email: 'old@example.com', display_name: 'Kept', role: 'admin' },
    ]);
    await ensureProfile(user);
    const profile = await getConnection().select('profiles', { id: user.id });
    assert.deepEqual([profile[0].display_name, profile[0].role], ['Kept', 'admin']);
  });

  it('is safe when two first requests create the same profile at once', async () => {
    const user = someone();
    await Promise.all([ensureProfile(user), ensureProfile({ ...user })]);
    assert.equal((await getConnection().select('profiles', { id: user.id })).length, 1);
  });

  it('writes once per user, not on every request', async () => {
    const user = someone();
    await ensureProfile(user);
    const upsert = mock.method(getConnection(), 'upsert');
    await ensureProfile(user);
    assert.equal(upsert.mock.callCount(), 0);
  });
});

describe('userAuthMiddleware and the profile', () => {
  /** Makes Supabase know the token's owner as `user`. */
  const supabaseKnows = (user: Record<string, unknown>) =>
    Object.assign(sessionUsers, {
      configured: () => true,
      getUser: async () => ({ data: { user: { ...user, factors: [] } }, error: null }),
    });
  const signIn = () =>
    runMiddleware(userAuthMiddleware, fakeRequest({ headers: { authorization: `Bearer ${tokenWithAal('aal1')}` } }));

  it('creates the profile of a person signing in for the first time', async () => {
    const user = someone();
    supabaseKnows(user);
    assert.equal((await signIn()).passed, true);
    assert.equal((await findProfile(user.id)).email, 'ada@example.com');
  });

  it('still lets the person in when the profile cannot be written', async () => {
    supabaseKnows(someone());
    mock.method(getConnection(), 'upsert', async () => {
      throw new Error('database down');
    });
    mock.method(console, 'error', () => {});
    assert.equal((await signIn()).passed, true);
  });
});
