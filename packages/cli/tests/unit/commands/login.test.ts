/**
 * Unit tests for `oya login` with flags (src/commands/login.ts): the CI path
 * proves the key before saving it, and never prompts.
 */
import { BASE, captured, fakeFetch, reply } from '../support/harness.ts';
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { cmdLogin } from '../../../src/commands/login.ts';
import { load } from '../../../src/config.ts';

describe('oya login --key --url', () => {
  afterEach(() => mock.restoreAll());

  it('verifies the key, then saves it with the URL trimmed', async () => {
    const calls = fakeFetch({ 'GET /api/config': {} });
    const { out } = await captured(() => cmdLogin({ key: 'k1', url: `${BASE}/` }));
    assert.deepEqual(
      calls.map((c) => c.path),
      ['/api/config'],
    );
    assert.deepEqual(load(), { apiKey: 'k1', baseUrl: BASE });
    assert.match(out, /✅ Signed in to http:\/\/oya.test\./);
  });

  it('refuses to save a key the control plane rejects', async () => {
    fakeFetch({ 'GET /api/config': reply(401, { error: 'no' }) });
    await assert.rejects(cmdLogin({ key: 'bad', url: BASE }), /That key was rejected by http:\/\/oya.test \(401\)/);
    assert.notEqual(load().apiKey, 'bad');
  });
});
