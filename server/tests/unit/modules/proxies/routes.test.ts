/**
 * Unit tests for the proxy REST routes, called in-process: each key sees and
 * removes only its own proxies, and no response or audit record carries a
 * proxy's credentials.
 */
import { describe, it, before, after, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-proxy-routes-');
const { router } = await import('../../../../src/modules/proxies/routes.ts');
const proxies = await import('../../../../src/modules/proxies/service.ts');
const store = await import('../../../../src/modules/proxies/store.ts');
const { Proxy } = await import('../../../../src/modules/proxies/proxy.ts');
const { recent, fingerprint } = await import('../../../../src/platform/audit.ts');
const { allowKey, callRoute } = await import('../../support/agent.ts');

const KEY = 'proxy-routes-key';
const OTHER = 'proxy-routes-other';
let forget: (() => void)[] = [];
/** Calls the router as `key`. */
const call = (method: string, url: string, body?: any, key = KEY) => callRoute(router, { method, url, body, key });

describe('proxy routes', () => {
  before(() => (forget = [allowKey(KEY), allowKey(OTHER)]));
  after(() => forget.forEach((f) => f()));
  beforeEach(() => proxies.reset());

  it('refuses a caller without an API key', async () => {
    assert.equal((await callRoute(router, { url: '/proxies' })).status, 401);
  });

  it('registers a proxy for the caller, answers 201 without credentials, and audits it', async () => {
    const res = await call('POST', '/proxies', { label: 'Mine', url: 'http://u:secret@8.8.8.8:3128', geo: 'US' });
    assert.equal(res.status, 201);
    assert.deepEqual([res.body.label, res.body.geo, res.body.shared], ['Mine', 'US', false]);
    assert.equal(store.proxies.get(res.body.id).owner, fingerprint(KEY));
    assert.doesNotMatch(JSON.stringify(res.body), /secret/);
    const audited = recent({ action: 'proxy.create' })[0];
    assert.deepEqual([audited.target_id, audited.meta], [res.body.id, { geo: 'US', kind: 'residential' }]);
    assert.doesNotMatch(JSON.stringify(audited), /secret|8\.8\.8\.8/);
  });

  it('answers a bad proxy URL with its 400', async () => {
    const res = await call('POST', '/proxies', { url: 'ftp://8.8.8.8' });
    assert.deepEqual([res.status, res.body.error], [400, 'proxy url must be http, https or socks5']);
  });

  it("lists the caller's and shared proxies only", async () => {
    store.proxies.set('px-mine', new Proxy({ id: 'px-mine', owner: fingerprint(KEY) }));
    store.proxies.set('px-theirs', new Proxy({ id: 'px-theirs', owner: fingerprint(OTHER) }));
    store.proxies.set('px-shared', new Proxy({ id: 'px-shared', owner: null }));
    const res = await call('GET', '/proxies');
    assert.deepEqual(
      res.body.proxies.map((p) => p.id),
      ['px-mine', 'px-shared'],
    );
  });

  it("deletes the caller's proxy, and answers 404 for another key's", async () => {
    store.proxies.set('px-mine', new Proxy({ id: 'px-mine', owner: fingerprint(KEY) }));
    const theirs = await call('DELETE', '/proxies/px-mine', undefined, OTHER);
    assert.deepEqual([theirs.status, theirs.body], [404, { error: 'No such proxy' }]);
    assert.deepEqual((await call('DELETE', '/proxies/px-mine')).body, { ok: true });
    assert.equal(recent({ action: 'proxy.delete' })[0].target_id, 'px-mine');
  });

  it('checks only the proxies the caller can see', async () => {
    const res = await call('POST', '/proxies/check');
    assert.deepEqual(res.body, { results: [] });
  });
});
