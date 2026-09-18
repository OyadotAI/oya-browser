/**
 * Unit tests for the /gateway REST routes, called in-process: sessions,
 * providers and strategy, profiles and recordings, each scoped to the
 * calling key so another key's items read as absent.
 */
import { describe, it, before, after, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-gateway-routes-');
const { router } = await import('../../../../src/modules/gateway/routes.ts');
const { sessions } = await import('../../../../src/modules/gateway/service.ts');
const { Session } = await import('../../../../src/modules/gateway/session.ts');
const { pool } = await import('../../../../src/modules/gateway/routing.ts');
const profiles = await import('../../../../src/modules/gateway/profiles.ts');
const { DIR, frameFile } = await import('../../../../src/modules/gateway/recording-live.ts');
const { recent, fingerprint } = await import('../../../../src/platform/audit.ts');
const { allowKey, callRoute } = await import('../../support/agent.ts');
const { stubControl, FakeWs } = await import('../../support/gateway.ts');

const KEY = 'gateway-routes-key';
const OTHER = 'gateway-routes-other';
const OWNER = fingerprint(KEY);
let forget: (() => void)[] = [];
/** Calls the router as `key`. */
const call = (method: string, url: string, body?: any, key = KEY) => callRoute(router, { method, url, body, key });
/** A live session for `apiKey`. */
function session(id: string, apiKey: string) {
  const s = new Session({
    id,
    apiKey,
    provider: 'chrome',
    release: async () => {},
    upstream: new FakeWs(),
    profile: null,
  });
  sessions.set(id, s);
  return s;
}
/** A spooled recording owned by `owner`, with one frame. */
function recording(owner: string) {
  const id = randomUUID();
  mkdirSync(join(DIR, id), { recursive: true });
  writeFileSync(
    join(DIR, id, 'manifest.json'),
    JSON.stringify({ sessionId: id, owner, startedAt: new Date().toISOString(), frames: [] }),
  );
  writeFileSync(join(DIR, id, frameFile(0)), 'jpeg');
  return id;
}

before(() => (forget = [allowKey(KEY), allowKey(OTHER)]));
after(() => forget.forEach((f) => f()));
afterEach(() => {
  mock.restoreAll();
  sessions.clear();
  for (const p of pool.visible(OWNER)) if (p.owner === OWNER) pool.remove(OWNER, p.name);
});

describe('gateway session routes', () => {
  it('refuses a caller without an API key', async () => {
    assert.equal((await callRoute(router, { url: '/gateway/sessions' })).status, 401);
  });

  it("lists the caller's sessions only", async () => {
    session('s-mine', KEY);
    session('s-theirs', OTHER);
    const res = await call('GET', '/gateway/sessions');
    assert.deepEqual(
      res.body.sessions.map((s) => s.id),
      ['s-mine'],
    );
  });

  it("kills the caller's session with the reason given, and audits it", async () => {
    stubControl();
    const s = session('s-mine', KEY);
    assert.deepEqual((await call('DELETE', '/gateway/sessions/s-mine', { reason: 'enough' })).body, { ok: true });
    assert.equal(s.closed, true);
    assert.equal(recent({ action: 'gateway.session.end' })[0].meta.reason, 'enough');
    assert.equal(recent({ action: 'gateway.session.kill' })[0].target_id, 's-mine');
  });

  it("answers 404 for another key's session, or one that does not exist", async () => {
    session('s-theirs', OTHER);
    for (const id of ['s-theirs', 's-missing']) {
      const res = await call('DELETE', `/gateway/sessions/${id}`);
      assert.deepEqual([res.status, res.body], [404, { error: 'No such session' }]);
    }
    assert.equal(sessions.get('s-theirs').closed, false);
  });
});

describe('gateway provider routes', () => {
  it("reports the caller's view of the pool", async () => {
    const res = await call('GET', '/gateway/providers');
    assert.equal(res.body.strategy, 'priority');
    assert.ok(Array.isArray(res.body.providers));
  });

  it('registers a provider for the caller and audits it', async () => {
    const res = await call('POST', '/gateway/providers', { name: 'home', wsUrl: 'ws://8.8.8.8:9222', priority: 3 });
    assert.deepEqual([res.status, res.body.name, res.body.owner], [200, 'home', OWNER]);
    assert.deepEqual(recent({ action: 'provider.upsert' })[0].meta, { type: 'cdp', maxConcurrent: 10, priority: 3 });
  });

  it('answers a refused registration with its status and message', async () => {
    const res = await call('POST', '/gateway/providers', { name: 'bad', wsUrl: 'http://x' });
    assert.equal(res.status, 400);
    assert.match(res.body.error, /ws:\/\/ or wss:\/\//);
  });

  it("removes the caller's provider, and answers 404 for anyone else's", async () => {
    await call('POST', '/gateway/providers', { name: 'home', wsUrl: 'ws://8.8.8.8:9222' });
    const theirs = await call('DELETE', '/gateway/providers/home', undefined, OTHER);
    assert.deepEqual([theirs.status, theirs.body], [404, { error: 'Provider not found.' }]);
    assert.deepEqual((await call('DELETE', '/gateway/providers/home')).body, { ok: true });
  });

  it("sets the caller's routing strategy, auditing the change", async () => {
    const res = await call('POST', '/gateway/strategy', { strategy: 'latency' });
    assert.deepEqual(res.body, { ok: true, strategy: 'latency' });
    assert.equal(pool.strategyFor(OWNER), 'latency');
    assert.equal(pool.strategyFor(fingerprint(OTHER)), pool.strategy);
    assert.deepEqual(recent({ action: 'routing.strategy' })[0].meta, { from: 'priority', to: 'latency' });
  });

  it('refuses an unknown strategy with a 400 listing the choices', async () => {
    const res = await call('POST', '/gateway/strategy', { strategy: 'fastest' });
    assert.deepEqual(
      [res.status, res.body.error],
      [400, 'strategy must be one of priority, round-robin, least-connections, latency, weighted'],
    );
  });
});

describe('gateway profile routes', () => {
  it("lists the caller's profiles", async () => {
    const res = await call('GET', '/gateway/profiles');
    assert.deepEqual(res.body, { profiles: [] });
  });

  it('deletes a profile, auditing whether there was one', async () => {
    assert.deepEqual((await call('DELETE', '/gateway/profiles/shop')).body, { ok: false });
    assert.equal(recent({ action: 'profile.delete' })[0].outcome, 'error');
  });

  it('refuses to delete a profile in use', async () => {
    profiles.tryLock(OWNER, 'shop');
    const res = await call('DELETE', '/gateway/profiles/shop');
    assert.deepEqual([res.status, res.body.error], [409, 'Profile is in use']);
    profiles.unlock(OWNER, 'shop');
  });
});

describe('gateway recording routes', () => {
  it("lists the caller's recordings only", async () => {
    const mine = recording(OWNER);
    recording(fingerprint(OTHER));
    const res = await call('GET', '/gateway/recordings');
    assert.deepEqual(
      res.body.recordings.map((r) => r.sessionId),
      [mine],
    );
  });

  it("serves the caller's manifest and frames, and 404s for anyone else", async () => {
    const id = recording(OWNER);
    assert.equal((await call('GET', `/gateway/recordings/${id}`)).body.sessionId, id);
    const frame = await call('GET', `/gateway/recordings/${id}/frames/0`);
    assert.equal(frame.headers['Content-Type'], 'image/jpeg');
    assert.equal(frame.body.toString(), 'jpeg');
    assert.deepEqual((await call('GET', `/gateway/recordings/${id}`, undefined, OTHER)).body, {
      error: 'No such recording',
    });
    assert.deepEqual((await call('GET', `/gateway/recordings/${id}/frames/0`, undefined, OTHER)).body, {
      error: 'No such frame',
    });
  });

  it("deletes the caller's recording, and not another key's", async () => {
    const id = recording(OWNER);
    assert.deepEqual((await call('DELETE', `/gateway/recordings/${id}`, undefined, OTHER)).body, { ok: false });
    assert.deepEqual((await call('DELETE', `/gateway/recordings/${id}`)).body, { ok: true });
    assert.equal(recent({ action: 'recording.delete' })[0].target_id, id);
  });
});
