/**
 * Unit tests for request authentication: which tokens are accepted and as
 * what, the role limits each credential carries, and a share link confined to
 * its own browser's endpoints.
 */
import { describe, it, afterEach, after, mock } from 'node:test';
import assert from 'node:assert/strict';
import { Status } from '../../../../src/platform/http-status.ts';
import { ownDataDir, restoreEnv } from '../../support/data-dir.ts';
import { fakeRequest, runMiddleware } from '../../support/auth.ts';

ownDataDir('oya-middleware-');
const savedKeys = process.env.API_KEYS;
process.env.API_KEYS = 'env-admin-key';
after(() => restoreEnv('API_KEYS', savedKeys));
const { authenticateToken, authMiddleware, userAuthMiddleware, shareReaches } =
  await import('../../../../src/modules/auth/middleware.ts');
const { mintImpersonation } = await import('../../../../src/modules/auth/impersonate.ts');
const { registerApiKey } = await import('../../../../src/modules/auth/api-keys.ts');
const { generateKey, keyDigest, validateApiKey } = await import('../../../../src/modules/auth/keys.ts');
const { recent } = await import('../../../../src/platform/audit.ts');
const { KEY_EXPIRED_AUDIT_MS } = await import('../../../../src/modules/auth/constants.ts');
const { control, projectId } = await import('../../../../src/modules/control/service.ts');
const { issue } = await import('../../../../src/modules/control/service/credentials.ts');

/** A stored key registered to a user, and its project. */
async function storedKey(userId = 'owner') {
  const key = generateKey();
  await registerApiKey(key, userId, 'Test');
  return key;
}

/** A project credential with this role on a fresh key. */
async function credential(role: string) {
  const key = await storedKey();
  return { key, token: (await control().credential(key, { role })).token };
}

/** A share link for one session, as a viewer or an operator. */
async function share(role: 'viewer' | 'operator', sessionId = 'sess-1') {
  const key = await storedKey();
  const grant = { role, label: 'share', expiresAt: null, memberUser: null, sessionId };
  return (await issue(control().store, key, grant)).token;
}

/** Runs authMiddleware for a bearer token. */
function admit(token: string, request: any = {}) {
  const req = fakeRequest({ ...request, headers: { authorization: `Bearer ${token}` } });
  return runMiddleware(authMiddleware, req).then((out) => ({ ...out, req }));
}

afterEach(() => {
  mock.restoreAll();
  mock.timers.reset();
});

describe('authenticateToken', () => {
  it('refuses a missing token with 401', async () => {
    await assert.rejects(authenticateToken(''), { status: Status.UNAUTHORIZED, message: 'Missing API key' });
  });

  it('refuses an unknown key with 403', async () => {
    await assert.rejects(authenticateToken(generateKey()), { status: Status.FORBIDDEN, message: 'Invalid API key' });
  });

  it('accepts an env key and a stored key as administrator', async () => {
    assert.deepEqual(await authenticateToken('env-admin-key'), { key: 'env-admin-key', role: 'administrator' });
    const key = await storedKey();
    assert.deepEqual(await authenticateToken(key), { key, role: 'administrator' });
  });

  it('refuses the key of a deleted project with 410', async () => {
    const key = await storedKey('deleter');
    await control().updateOwnedProject('deleter', projectId(key), { remove: true });
    await assert.rejects(authenticateToken(key), { status: Status.GONE, message: 'Project has been deleted' });
  });

  it('resolves a scoped credential to its project key and role', async () => {
    const { key, token } = await credential('viewer');
    const principal = await authenticateToken(token);
    assert.deepEqual([principal.key, principal.role], [key, 'viewer']);
  });

  it('treats an oya_ token the control plane does not know as an unknown key', async () => {
    await assert.rejects(authenticateToken('oya_not-issued'), { status: Status.FORBIDDEN, message: 'Invalid API key' });
  });

  it('accepts a managed browser’s credential only where browsers are allowed', async () => {
    const browser = { key: 'k', role: 'browser', sessionId: 's' };
    mock.method(control(), 'authenticate', async () => browser);
    await assert.rejects(authenticateToken('oya_browser'), {
      status: Status.FORBIDDEN,
      message: 'Managed browser credentials cannot call this API',
    });
    assert.equal(await authenticateToken('oya_browser', { allowBrowser: true }), browser);
  });
});

describe('an expiring API key', () => {
  /** A stored key that expires at `expiresAt`. */
  async function keyExpiring(expiresAt: string) {
    const key = generateKey();
    await registerApiKey(key, 'owner', 'Test', expiresAt);
    return key;
  }

  /** The key.expired audit rows for a key. */
  const expiredRows = (key: string) =>
    recent({ action: 'key.expired' }).filter((r: any) => r.target_id === keyDigest(key));

  it('works until its expiry', async () => {
    const key = await keyExpiring(new Date(Date.now() + 60_000).toISOString());
    assert.deepEqual(await authenticateToken(key), { key, role: 'administrator' });
  });

  it('is refused with 401 and the day it expired once past it', async () => {
    const key = await keyExpiring('2020-05-06T00:00:00.000Z');
    await assert.rejects(authenticateToken(key), {
      status: Status.UNAUTHORIZED,
      message: 'API key expired on 2020-05-06; create a new key in the console',
    });
  });

  it('is refused once it expires, though it was valid and cached a moment before', async () => {
    mock.timers.enable({ apis: ['Date'], now: Date.now() });
    const key = await keyExpiring(new Date(Date.now() + 60_000).toISOString());
    await authenticateToken(key);
    assert.equal(validateApiKey(key), true);
    mock.timers.tick(60_000);
    assert.equal(validateApiKey(key), false, 'the cache-only check honours expiry');
    await assert.rejects(authenticateToken(key), { status: Status.UNAUTHORIZED });
  });

  it('is answered 401 by the API middleware', async () => {
    const out = await admit(await keyExpiring('2020-05-06T00:00:00.000Z'));
    assert.equal(out.res.statusCode, Status.UNAUTHORIZED);
  });

  it('audits a refusal as key.expired once per key per hour, however often the client retries', async () => {
    mock.timers.enable({ apis: ['Date'], now: Date.now() });
    const key = await keyExpiring('2020-05-06T00:00:00.000Z');
    for (let i = 0; i < 3; i++) await authenticateToken(key).catch(() => {});
    assert.equal(expiredRows(key).length, 1);
    mock.timers.tick(KEY_EXPIRED_AUDIT_MS);
    await authenticateToken(key).catch(() => {});
    assert.equal(expiredRows(key).length, 2);
  });
});

describe('authMiddleware', () => {
  it('passes the request on under the principal’s key', async () => {
    const { key, token } = await credential('operator');
    const { passed, req } = await admit(token, { method: 'GET', path: '/browsers' });
    assert.equal(passed, true);
    assert.equal(req.headers.authorization, `Bearer ${key}`);
    assert.equal(req.authToken, token);
    assert.equal(req.principal.role, 'operator');
  });

  it('prefers a token the router already found', async () => {
    const req = fakeRequest({ path: '/browsers' });
    req.authToken = 'env-admin-key';
    assert.equal((await runMiddleware(authMiddleware, req)).passed, true);
  });

  it('answers an authentication failure with its status', async () => {
    const { res, passed } = await runMiddleware(authMiddleware, fakeRequest());
    assert.equal(passed, false);
    assert.deepEqual([res.statusCode, res.body], [Status.UNAUTHORIZED, { error: 'Missing API key' }]);
    assert.equal((await admit('not-a-key')).res.statusCode, Status.FORBIDDEN);
  });

  it('lets a viewer read', async () => {
    const { token } = await credential('viewer');
    assert.equal((await admit(token, { method: 'GET', path: '/browsers' })).passed, true);
    assert.equal((await admit(token, { method: 'HEAD', path: '/personas' })).passed, true);
  });

  it('refuses a viewer any change', async () => {
    const { token } = await credential('viewer');
    const { res, passed } = await admit(token, { method: 'POST', path: '/browsers' });
    assert.equal(passed, false);
    assert.deepEqual(
      [res.statusCode, res.body.error],
      [Status.FORBIDDEN, 'Viewer credentials cannot change resources'],
    );
  });

  it('hides secrets, cookies, live views and recordings from a viewer', async () => {
    const { token } = await credential('viewer');
    for (const path of [
      '/config',
      '/pool/cookies',
      '/live/b1',
      '/gateway/profiles',
      '/gateway/recordings/r',
      '/CONFIG',
    ]) {
      const { res } = await admit(token, { method: 'GET', path });
      assert.equal(res.body?.error, 'Operator permission required', path);
    }
  });

  it('keeps settings writes for administrators', async () => {
    const { token } = await credential('operator');
    for (const path of [
      '/config',
      '/personas/p1',
      '/proxies',
      '/gateway/providers',
      '/gateway/strategy',
      '/Personas',
    ]) {
      const { res } = await admit(token, { method: 'PUT', path });
      assert.equal(res.body?.error, 'Administrator permission required', path);
    }
  });

  it('lets an operator read settings and act on browsers', async () => {
    const { token } = await credential('operator');
    assert.equal((await admit(token, { method: 'GET', path: '/personas' })).passed, true);
    assert.equal((await admit(token, { method: 'POST', path: '/browsers/b1/command' })).passed, true);
  });

  it('lets an administrator change settings', async () => {
    const { token } = await credential('administrator');
    assert.equal((await admit(token, { method: 'POST', path: '/config' })).passed, true);
    assert.equal((await admit('env-admin-key', { method: 'DELETE', path: '/personas/p1' })).passed, true);
  });
});

describe('shareReaches', () => {
  it('lets a share reach only its own session', () => {
    const share = { key: 'k', role: 'operator', sessionId: 's-1' };
    assert.deepEqual(
      ['s-1', 's-2', null, undefined].map((target) => shareReaches(share, target)),
      [true, false, false, false],
    );
  });

  it('never confines project credentials or a managed browser', () => {
    assert.equal(shareReaches({ key: 'k', role: 'operator' }, null), true);
    assert.equal(shareReaches({ key: 'k', role: 'browser', sessionId: 's-1' }, null), true);
  });
});

describe('authMiddleware with a share link', () => {
  /** Runs a share token against a path under /api. */
  const at = (token: string, method: string, path: string) => admit(token, { method, baseUrl: '/api', path });
  const REFUSED = 'This link only grants access to its shared browser';

  it('reads its own browser’s live view, status and session', async () => {
    const token = await share('viewer');
    for (const path of ['/live/sess-1', '/browsers/sess-1', '/control/sessions/sess-1']) {
      assert.equal((await at(token, 'GET', path)).passed, true, path);
    }
  });

  it('mints a stream ticket for its own session', async () => {
    const token = await share('viewer');
    assert.equal((await at(token, 'POST', '/control/sessions/sess-1/ticket')).passed, true);
  });

  it('reaches nothing else in the project', async () => {
    const token = await share('operator');
    for (const [method, path] of [
      ['GET', '/live/sess-2'],
      ['GET', '/browsers'],
      ['GET', '/browsers/sess-1/cookies'],
      ['POST', '/live/sess-1'],
      ['GET', '/control/sessions/sess-1/ticket'],
      ['DELETE', '/control/sessions/sess-1'],
    ]) {
      const { res, passed } = await at(token, method, path);
      assert.equal(passed, false, `${method} ${path}`);
      assert.equal(res.body.error, REFUSED);
    }
  });

  it('acts on its browser only when it is a control share', async () => {
    const view = await share('viewer');
    const control = await share('operator');
    for (const path of ['/control/sessions/sess-1/input', '/control/sessions/sess-1/control']) {
      assert.equal((await at(view, 'POST', path)).res.body?.error, REFUSED, path);
      assert.equal((await at(control, 'POST', path)).passed, true, path);
    }
  });

  it('matches the path after decoding it', async () => {
    const token = await share('viewer', 'sess 1');
    assert.equal((await at(token, 'GET', '/live/sess%201')).passed, true);
  });
});

describe('userAuthMiddleware', () => {
  it('refuses a request without a bearer token', async () => {
    const { res, passed } = await runMiddleware(
      userAuthMiddleware,
      fakeRequest({ headers: { authorization: 'Basic x' } }),
    );
    assert.equal(passed, false);
    assert.deepEqual([res.statusCode, res.body], [Status.UNAUTHORIZED, { error: 'Missing token' }]);
  });

  it('answers 503 when accounts are not configured', async () => {
    const req = fakeRequest({ headers: { authorization: 'Bearer jwt' } });
    const { res } = await runMiddleware(userAuthMiddleware, req);
    assert.deepEqual([res.statusCode, res.body], [Status.UNAVAILABLE, { error: 'Auth not configured' }]);
  });

  it('refuses a forged Login as token with 401, even beside a bearer token', async () => {
    const req = fakeRequest({ headers: { authorization: 'Bearer jwt', 'x-impersonate-token': 'forged' } });
    const { res, passed } = await runMiddleware(userAuthMiddleware, req);
    assert.equal(passed, false);
    assert.equal(res.statusCode, Status.UNAUTHORIZED);
    assert.equal(req.user, undefined);
  });

  it('takes a genuine Login as token over the bearer token, and needs Supabase Auth to find its user', async () => {
    process.env.OYA_IMPERSONATE_SECRET = 'x'.repeat(32);
    const token = mintImpersonation('cust', 'admin');
    const req = fakeRequest({ headers: { 'x-impersonate-token': token } });
    const { res, passed } = await runMiddleware(userAuthMiddleware, req);
    delete process.env.OYA_IMPERSONATE_SECRET;
    assert.deepEqual([passed, res.statusCode], [false, Status.UNAVAILABLE]);
  });
});
