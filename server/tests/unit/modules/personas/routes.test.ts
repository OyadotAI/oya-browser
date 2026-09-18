/**
 * Unit tests for the persona REST routes (routes.ts, which mounts
 * persona-routes.ts and site-routes.ts over route-support.ts), called
 * in-process: ownership answers 404, the device can never be edited, and
 * secrets are write-only.
 */
import { describe, it, before, after, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-persona-routes-');
const { personaRoutes } = await import('../../../../src/modules/personas/routes.ts');
const proxyStore = await import('../../../../src/modules/proxies/store.ts');
const { Proxy } = await import('../../../../src/modules/proxies/proxy.ts');
const credentials = await import('../../../../src/modules/personas/credentials.ts');
const mfa = await import('../../../../src/modules/challenges/mfa.ts');
const { recent, fingerprint } = await import('../../../../src/platform/audit.ts');
const { allowKey, callRoute } = await import('../../support/agent.ts');
const { personaService } = await import('../../support/personas.ts');

const KEY = 'persona-routes-key';
const OTHER = 'persona-routes-other';
let forget: (() => void)[] = [];
let service;
let router;

/** Calls the router as `key` (KEY unless given). */
const call = (method: string, url: string, body?: any, key = KEY) => callRoute(router, { method, url, body, key });
/** The newest audit record for an action. */
const lastAudit = (action: string) => recent({ action })[0];

describe('persona routes', () => {
  before(() => (forget = [allowKey(KEY), allowKey(OTHER)]));
  after(() => forget.forEach((f) => f()));
  beforeEach(() => {
    mock.method(console, 'log', () => {});
    ({ service } = personaService());
    router = personaRoutes(service);
    proxyStore.proxies.clear();
    proxyStore.assignments.clear();
    credentials.reset();
    mfa.reset();
  });

  it('refuses a caller without an API key', async () => {
    assert.equal((await callRoute(personaRoutes(service), { url: '/personas' })).status, 401);
  });

  it("lists the caller's personas, the default included, described without seeds", async () => {
    const res = await call('GET', '/personas');
    assert.equal(res.status, 200);
    assert.deepEqual(
      res.body.personas.map((p) => p.isDefault),
      [true],
    );
    assert.equal('seed' in res.body.personas[0], false);
    assert.ok(res.body.personas[0].fingerprint.platform);
  });

  it('serves the device choices a persona may make', async () => {
    const res = await call('GET', '/personas/options');
    assert.ok(res.body.platforms.includes('MacIntel'));
  });

  it('previews a fingerprint without creating a persona', async () => {
    const res = await call('POST', '/personas/preview', { prefs: { platform: 'MacIntel' } });
    assert.equal(res.body.fingerprint.platform, 'MacIntel');
    assert.equal(service.list(KEY).length, 1);
  });

  it('refuses a preview or creation that asks for a device not offered, with a 400', async () => {
    for (const url of ['/personas/preview', '/personas']) {
      const res = await call('POST', url, { prefs: { timezone: 'Mars/Olympus' } });
      assert.equal(res.status, 400);
      assert.match(res.body.error, /timezone "Mars\/Olympus" is not offered/);
    }
    assert.equal(service.list(KEY).length, 1);
  });

  it('creates a persona with the device asked for, answers 201 and audits it', async () => {
    const res = await call('POST', '/personas', {
      name: 'Work',
      prefs: { platform: 'Linux x86_64' },
      maxConcurrent: 3,
    });
    assert.equal(res.status, 201);
    assert.deepEqual(
      [res.body.name, res.body.prefs, res.body.maxConcurrent],
      ['Work', { platform: 'Linux x86_64' }, 3],
    );
    assert.equal(res.body.fingerprint.platform, 'Linux x86_64');
    assert.equal(lastAudit('persona.create').target_id, res.body.id);
  });

  it('reads one persona, and answers 404 for one the caller does not own', async () => {
    const p = service.create(KEY);
    assert.equal((await call('GET', `/personas/${p.id}`)).body.id, p.id);
    const theirs = await call('GET', `/personas/${p.id}`, undefined, OTHER);
    assert.deepEqual([theirs.status, theirs.body], [404, { error: 'No such persona' }]);
  });

  it('changes the name, cap and proxy', async () => {
    const p = service.create(KEY);
    const res = await call('PUT', `/personas/${p.id}`, { name: 'New', maxConcurrent: 4 });
    assert.deepEqual([res.status, res.body.name, res.body.maxConcurrent], [200, 'New', 4]);
    assert.deepEqual(lastAudit('persona.update').meta, { fields: ['name', 'maxConcurrent'] });
  });

  it('refuses an update touching the device or identity, naming the fields, with a 400', async () => {
    const p = service.create(KEY);
    const seed = p.seed;
    const res = await call('PUT', `/personas/${p.id}`, { name: 'x', seed: 1, prefs: {}, fingerprint: {} });
    assert.equal(res.status, 400);
    assert.match(
      res.body.error,
      /^seed, prefs, fingerprint cannot change after creation .* Clone it for a different device\.$/,
    );
    assert.deepEqual([p.seed, p.name], [seed, p.id]);
  });

  it("answers 404 for an update to another key's persona", async () => {
    const p = service.create(OTHER);
    assert.equal((await call('PUT', `/personas/${p.id}`, { name: 'x' })).status, 404);
  });

  it('clones a persona onto a fresh seed and answers 201', async () => {
    const p = service.create(KEY, { prefs: { platform: 'MacIntel' } });
    const res = await call('POST', `/personas/${p.id}/clone`, { name: 'Twin' });
    assert.equal(res.status, 201);
    assert.deepEqual([res.body.name, res.body.prefs], ['Twin', { platform: 'MacIntel' }]);
    assert.notEqual(res.body.id, p.id);
    assert.deepEqual(lastAudit('persona.create').meta, { name: 'Twin', clonedFrom: p.id });
  });

  it("answers 404 when cloning another key's persona", async () => {
    const p = service.create(OTHER);
    assert.equal((await call('POST', `/personas/${p.id}/clone`, {})).status, 404);
  });

  it("pins a persona to one of the caller's proxies, and unpins it", async () => {
    const p = service.create(KEY);
    proxyStore.proxies.set('px-mine', new Proxy({ id: 'px-mine', owner: fingerprint(KEY), sealed: 'x' }));
    const pinned = await call('PUT', `/personas/${p.id}/proxy`, { proxyId: 'px-mine' });
    assert.deepEqual([pinned.body.ok, pinned.body.proxy.id], [true, 'px-mine']);
    const unpinned = await call('PUT', `/personas/${p.id}/proxy`, { proxyId: null });
    assert.deepEqual(unpinned.body, { ok: true, proxy: null });
  });

  it("refuses to pin to another key's proxy, or one that does not exist, with a 404", async () => {
    const p = service.create(KEY);
    proxyStore.proxies.set('px-theirs', new Proxy({ id: 'px-theirs', owner: fingerprint(OTHER), sealed: 'x' }));
    for (const proxyId of ['px-theirs', 'px-missing']) {
      const res = await call('PUT', `/personas/${p.id}/proxy`, { proxyId });
      assert.deepEqual([res.status, res.body], [404, { error: 'No such proxy' }]);
    }
  });

  it("answers 404 when pinning another key's persona", async () => {
    const p = service.create(OTHER);
    assert.equal((await call('PUT', `/personas/${p.id}/proxy`, { proxyId: null })).status, 404);
  });

  it('deletes a persona and audits it', async () => {
    const p = service.create(KEY);
    const res = await call('DELETE', `/personas/${p.id}`);
    assert.deepEqual(res.body, { ok: true });
    assert.equal(service.get(KEY, p.id), null);
    assert.equal(lastAudit('persona.delete').target_id, p.id);
  });

  it('refuses to delete the default persona, and answers 404 for a missing one', async () => {
    const d = service.defaultFor(KEY);
    const res = await call('DELETE', `/personas/${d.id}`);
    assert.deepEqual([res.status, res.body.error], [400, 'The default persona cannot be deleted']);
    assert.equal((await call('DELETE', '/personas/p-missing')).status, 404);
  });
});

describe('persona site routes', () => {
  before(() => (forget = [allowKey(KEY), allowKey(OTHER)]));
  after(() => forget.forEach((f) => f()));
  beforeEach(() => {
    mock.method(console, 'log', () => {});
    ({ service } = personaService());
    router = personaRoutes(service);
    credentials.reset();
    mfa.reset();
  });

  it('stores a site login and answers with the username, never the password', async () => {
    const p = service.create(KEY);
    const res = await call('PUT', `/personas/${p.id}/credentials`, {
      domain: 'https://www.a.com',
      username: 'me',
      password: 'pw',
    });
    assert.deepEqual(res.body, { configured: true, domain: 'a.com', username: 'me' });
    assert.deepEqual(lastAudit('credentials.configure').meta, { domain: 'a.com', username: 'me' });
    assert.doesNotMatch(JSON.stringify(recent({})), /"pw"/);
  });

  it('lists site logins by username only', async () => {
    const p = service.create(KEY);
    credentials.set(p.id, 'a.com', { username: 'me', password: 'pw' });
    const res = await call('GET', `/personas/${p.id}/credentials`);
    assert.deepEqual(res.body, { credentials: [{ domain: 'a.com', username: 'me' }] });
  });

  it('forgets the login for one site, and requires the domain', async () => {
    const p = service.create(KEY);
    credentials.set(p.id, 'a.com', { username: 'me', password: 'pw' });
    assert.deepEqual((await call('DELETE', `/personas/${p.id}/credentials?domain=a.com`)).body, { ok: true });
    assert.deepEqual((await call('DELETE', `/personas/${p.id}/credentials?domain=a.com`)).body, { ok: false });
    const missing = await call('DELETE', `/personas/${p.id}/credentials`);
    assert.deepEqual([missing.status, missing.body.error], [400, 'a domain query parameter is required']);
  });

  it("answers 404 for the credentials of another key's persona", async () => {
    const p = service.create(OTHER);
    assert.equal((await call('GET', `/personas/${p.id}/credentials`)).status, 404);
    assert.equal((await call('PUT', `/personas/${p.id}/credentials`, {})).status, 404);
    assert.equal((await call('DELETE', `/personas/${p.id}/credentials?domain=a.com`)).status, 404);
  });

  it('configures a second factor for one site without echoing the secret', async () => {
    const p = service.create(KEY);
    const res = await call('PUT', `/personas/${p.id}/mfa`, {
      type: 'totp',
      secret: 'JBSWY3DPEHPK3PXP',
      domain: 'www.a.com',
    });
    assert.equal(res.status, 200);
    assert.doesNotMatch(JSON.stringify(res.body), /JBSWY3DPEHPK3PXP/);
    assert.deepEqual(lastAudit('mfa.configure').meta, { type: 'totp', domain: 'a.com' });
  });

  it('refuses a second factor for a domain that is not a hostname', async () => {
    const p = service.create(KEY);
    const res = await call('PUT', `/personas/${p.id}/mfa`, { type: 'totp', secret: 'x', domain: 'a b' });
    assert.deepEqual([res.status, res.body.error], [400, 'domain is not a hostname']);
  });

  it('clears a second factor, for one site or persona-wide', async () => {
    const p = service.create(KEY);
    assert.deepEqual((await call('DELETE', `/personas/${p.id}/mfa?domain=a.com`)).body, { ok: true });
    assert.deepEqual(lastAudit('mfa.clear').meta, { domain: 'a.com' });
    await call('DELETE', `/personas/${p.id}/mfa`);
    assert.deepEqual(lastAudit('mfa.clear').meta, { domain: null });
  });

  it("answers 404 for the second factor of another key's persona", async () => {
    const p = service.create(OTHER);
    assert.equal((await call('PUT', `/personas/${p.id}/mfa`, { type: 'totp', secret: 'x' })).status, 404);
    assert.equal((await call('DELETE', `/personas/${p.id}/mfa`)).status, 404);
  });
});
