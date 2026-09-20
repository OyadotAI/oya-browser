/**
 * Unit tests for the pool route handlers: a round-robin command that names
 * the browser that ran it, and clearing a persona's cookie jar.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { clearJar, poolCommand, queryPersona } from '../../../../../src/modules/browsers/http/pool.ts';
import { container } from '../../../../../src/app/container.ts';
import { recent } from '../../../../../src/platform/audit.ts';
import { disconnectBrowser } from '../../../support/fakes.ts';
import { FakeResponse, driveBrowser, fakeRequest, stubControl } from '../../../support/browsers.ts';

const B = 'b-poolcmd';

/** Sends a pool command as `key` and returns the response. */
async function command(key: string, body: object) {
  const res = new FakeResponse();
  await poolCommand(fakeRequest({ key, body }), res);
  return res;
}

describe('poolCommand', () => {
  beforeEach(() => stubControl());
  afterEach(() => {
    mock.restoreAll();
    disconnectBrowser(B);
  });

  it('runs the command on a pool browser and names it in the answer', async () => {
    driveBrowser(B, () => ({ ok: true, data: 1 }), 'k-pool');
    const res = await command('k-pool', { action: 'scroll' });
    assert.deepEqual(res.body, { ok: true, data: 1, _browser: B });
  });

  it('answers 503 when the key has no browsers', async () => {
    const res = await command('k-empty', { action: 'click' });
    assert.deepEqual([res.statusCode, res.body], [503, { error: 'No browsers available in pool' }]);
  });

  it('refuses a server-internal action, just as the browser route does', async () => {
    driveBrowser(B, () => ({ ok: true }), 'k-pool');
    assert.equal((await command('k-pool', { action: 'evaluate' })).statusCode, 403);
  });

  it('answers 500 naming the browser when the command throws', async () => {
    driveBrowser(B, () => Promise.reject(new Error('target crashed')), 'k-pool');
    const res = await command('k-pool', { action: 'click' });
    assert.deepEqual([res.statusCode, res.body], [500, { ok: false, error: 'target crashed', _browser: B }]);
  });
});

describe('queryPersona', () => {
  it('reads ?persona= as one id, and nothing when absent', () => {
    assert.equal(queryPersona({ query: { persona: 'p1' } }), 'p1');
    assert.equal(queryPersona({ query: {} }), undefined);
  });

  it('turns a repeated param into an id that cannot match, never the default', () => {
    assert.equal(queryPersona({ query: { persona: ['a', 'b'] } }), 'a,b');
  });
});

describe('clearJar', () => {
  afterEach(() => mock.restoreAll());

  it('clears the default persona’s jar and audits it', () => {
    const res = new FakeResponse();
    clearJar(fakeRequest({ key: 'k-jar' }), res);
    const persona = container.personas.defaultFor('k-jar');
    assert.deepEqual(res.body, { ok: true, persona: persona.id });
    assert.equal(recent({ action: 'cookies.clear' })[0].target_id, persona.id);
  });

  it('answers 404 for a persona the key does not own', () => {
    const res = new FakeResponse();
    clearJar(fakeRequest({ key: 'k-jar', query: { persona: 'p-not-mine' } }), res);
    assert.equal(res.statusCode, 404);
  });

  it('answers 500 when resolving fails without a status', () => {
    mock.method(container.personas, 'resolve', () => {
      throw new Error('store offline');
    });
    const res = new FakeResponse();
    clearJar(fakeRequest(), res);
    assert.deepEqual([res.statusCode, res.body], [500, { error: 'store offline' }]);
  });
});
