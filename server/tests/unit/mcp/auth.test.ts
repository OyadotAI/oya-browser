/**
 * Unit tests for MCP authorization: only an operator credential gets through;
 * a viewer is refused 403, a bad or missing key 401, and an unavailable
 * credential store 503.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { authorize } from '../../../src/mcp/auth.ts';
import { HttpError } from '../../../src/platform/errors.ts';
import { FakeResponse, fakeRequest, stubControl } from '../support/browsers.ts';

/** Authorizes a request carrying `key`; returns the outcome and the response. */
async function check(key: string) {
  const res = new FakeResponse();
  return { caller: await authorize(fakeRequest({ key }), res), res };
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

  it('answers 401 when no key is sent', async () => {
    const { caller, res } = await check('');
    assert.equal(caller, null);
    assert.deepEqual([res.statusCode, res.body], [401, { error: 'Missing or invalid API key' }]);
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
