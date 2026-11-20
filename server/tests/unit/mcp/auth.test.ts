/**
 * Unit tests for MCP authorization: only an operator credential gets through;
 * a viewer is refused 403, a bad or missing key 401, and an unavailable
 * credential store 503. A share link reaches only its own browser. With OAuth
 * on, a 401 names where an OAuth client signs in.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { authorize } from '../../../src/mcp/auth.ts';
import { HttpError } from '../../../src/platform/errors.ts';
import { FakeResponse, fakeRequest, stubControl } from '../support/browsers.ts';
import { restoreEnv } from '../support/data-dir.ts';
import { oauthRouter } from '../../../src/mcp/oauth/index.ts';

/** Authorizes a request carrying `key`; returns the outcome and the response. */
async function check(key: string, params = {}) {
  const res = new FakeResponse();
  return { caller: await authorize(fakeRequest({ key, params }), res), res };
}

describe('authorize', () => {
  afterEach(() => mock.restoreAll());

  it('lets an operator credential through with its key', async () => {
    stubControl({ authenticate: async () => ({ key: 'k-op', role: 'operator' }) });
    assert.deepEqual((await check('oya_operator')).caller, { key: 'k-op' });
  });

  it('refuses a viewer with 403', async () => {
    stubControl({ authenticate: async () => ({ key: 'k-view', role: 'viewer' }) });
    const { caller, res } = await check('oya_viewer');
    assert.equal(caller, null);
    assert.deepEqual([res.statusCode, res.body], [403, { error: 'Operator permission required' }]);
  });

  it('confines a share link to its own browser, never the pool', async () => {
    stubControl({ authenticate: async () => ({ key: 'k-op', role: 'operator', sessionId: 'b-1' }) });
    assert.deepEqual((await check('oya_share', { browserId: 'b-1' })).caller, { key: 'k-op', share: true });
    for (const params of [{}, { browserId: 'b-2' }]) {
      const { caller, res } = await check('oya_share', params);
      assert.equal(caller, null);
      assert.equal(res.statusCode, 403);
    }
  });

  it('answers 401 when no key is sent', async () => {
    const { caller, res } = await check('');
    assert.equal(caller, null);
    assert.deepEqual([res.statusCode, res.body], [401, { error: 'Missing or invalid API key' }]);
  });

  it('tells an OAuth client where to sign in when OAuth is on', async () => {
    const saved = process.env.OYA_CONSOLE_URL;
    process.env.OYA_CONSOLE_URL = 'https://oya.example';
    oauthRouter();
    restoreEnv('OYA_CONSOLE_URL', saved);
    const { res } = await check('');
    assert.match(res.headers['WWW-Authenticate'], /resource_metadata=".*\/oauth-protected-resource\/mcp\/pool"/);
  });

  it('answers 503, not 401, when credentials cannot be checked', async () => {
    stubControl({
      authenticate: async () => {
        throw new HttpError(503, 'store down');
      },
    });
    assert.equal((await check('oya_any')).res.statusCode, 503);
  });
});
