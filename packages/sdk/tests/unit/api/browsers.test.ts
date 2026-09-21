/**
 * Unit tests for `oya.browser` (src/api/browsers.ts, src/api/ready.ts):
 * starting, waiting for a cloud browser, reattaching and stopping.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { client, tickUntil } from '../support/fake-fetch.ts';

const START = 'POST /api/browsers/start';
const starting = { body: { id: 'c1', provider: 'oya-cloud', persona: 'p', status: 'starting' } };

describe('oya.browser.start', () => {
  beforeEach(() => mock.timers.enable({ apis: ['setTimeout', 'Date'] }));
  afterEach(() => mock.timers.reset());

  it('sends the options with persona as the profile, and an idempotency key', async () => {
    const { oya, calls } = client({
      [START]: { body: { id: 'b1', provider: 'steel', persona: 'auto', status: 'ready' } },
    });
    const b = await oya.browser.start({ persona: 'auto', queueMs: 5, governed: true });
    assert.equal(b.id, 'b1');
    assert.deepEqual(calls[0].body, { profile: 'auto', queueMs: 5, governed: true });
    assert.match(calls[0].headers['Idempotency-Key'], /^[0-9a-f-]{36}$/);
  });

  it('uses the caller idempotency key when given', async () => {
    const { oya, calls } = client({ [START]: { body: { id: 'b1', status: 'ready' } } });
    await oya.browser.start({ idempotencyKey: 'retry-1' });
    assert.equal(calls[0].headers['Idempotency-Key'], 'retry-1');
  });

  it('waits for a starting browser to dial in, then fetches its CDP URL', async () => {
    const { oya, calls } = client({
      [START]: starting,
      'GET /api/browsers': [{ body: [] }, { body: [{ id: 'c1', health: 'ok' }] }],
      'GET /api/control/sessions/c1': { status: 404, body: { error: 'unknown' } },
      'GET /api/browsers/c1': { body: { id: 'c1', cdpUrl: 'wss://c1' } },
    });
    const started = oya.browser.start();
    await tickUntil(started, 2000);
    assert.equal((await started).cdpUrl, 'wss://c1');
    assert.deepEqual(calls.map((c) => c.path).slice(-2), ['/api/browsers', '/api/browsers/c1']);
  });

  it('stops waiting with a 409 when the session ended', async () => {
    const { oya } = client({
      [START]: starting,
      'GET /api/browsers': { body: [{ id: 'c1', health: 'dead' }] },
      'GET /api/control/sessions/c1': { body: { id: 'c1', state: 'failed' } },
    });
    await assert.rejects(oya.browser.start(), { status: 409, message: 'Browser creation ended in failed' });
  });

  it('passes on a session lookup error other than 404', async () => {
    const { oya } = client({
      [START]: starting,
      'GET /api/browsers': { body: [] },
      'GET /api/control/sessions/c1': { status: 500, body: { error: 'db down' } },
    });
    await assert.rejects(oya.browser.start(), { status: 500, message: 'db down' });
  });

  it('gives up with a 504 after the ready timeout', async () => {
    const { oya } = client({
      [START]: starting,
      'GET /api/browsers': { body: [] },
      'GET /api/control/sessions/c1': { body: { id: 'c1', state: 'provisioning' } },
    });
    const started = oya.browser.start({ readyTimeoutMs: 6000 });
    await tickUntil(started, 2000);
    await assert.rejects(started, { status: 504, message: 'Browser c1 did not come up within 6s' });
  });
});

describe('oya.browser', () => {
  it('get reattaches with cdp and default persona when the record has none', async () => {
    const { oya } = client({ 'GET /api/browsers/a%2Fb': { body: { id: 'a/b', provider: null, persona: null } } });
    const b = await oya.browser.get('a/b');
    assert.deepEqual([b.id, b.provider, b.persona], ['a/b', 'cdp', 'default']);
  });

  it('stop sends ids, stop("all") and stopAll send all', async () => {
    const { oya, calls } = client({ 'POST /api/browsers/stop': { body: { stopped: 2, results: [] } } });
    await oya.browser.stop(['a', 'b']);
    await oya.browser.stop('all');
    assert.equal(await oya.browser.stopAll(), 2);
    assert.deepEqual(
      calls.map((c) => c.body),
      [{ ids: ['a', 'b'] }, { all: true }, { all: true }],
    );
  });
});
