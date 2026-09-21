/**
 * Unit tests for the fleet routes: health, operator-only provisioning, metrics
 * and drain, and the per-key fleet view, usage and audit trail.
 */
import { describe, it, before, after, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const { router } = await import('../../../../src/modules/fleet/routes.ts');
const { registry } = await import('../../../../src/modules/browsers/registry.ts');
const { audit, fingerprint } = await import('../../../../src/platform/audit.ts');
const { MAX_AUDIT_LIMIT } = await import('../../../../src/modules/fleet/constants.ts');
const { allowKey, callRoute } = await import('../../support/agent.ts');
const { connectBrowser, disconnectBrowser } = await import('../../support/fakes.ts');

const KEY = 'fleet-routes-key';
const OPERATOR = 'fleet-operator-token';
let forget: () => void;

/** Calls the router as the test key. */
const as = (method: string, url: string, key = KEY, body?: any) => callRoute(router, { method, url, key, body });

describe('fleet routes', () => {
  before(() => {
    forget = allowKey(KEY);
    process.env.OYA_OPERATOR_TOKEN = OPERATOR;
  });
  after(() => {
    forget();
    delete process.env.OYA_OPERATOR_TOKEN;
    registry.draining = false;
  });
  afterEach(() => disconnectBrowser('b-fleet'));

  it('answers health without a credential, counting connected browsers', async () => {
    connectBrowser('b-fleet', KEY);
    const res = await callRoute(router, { url: '/health' });
    assert.equal(res.body.status, 'ok');
    assert.ok(res.body.browsers >= 1);
    assert.equal(typeof res.body.uptime, 'number');
  });

  it('refuses provisioning to a tenant key', async () => {
    const res = await as('POST', '/fleet/provision?count=2');
    assert.equal(res.status, 403);
  });

  it('provisions the requested number of keys for the operator, at least one', async () => {
    const two = await as('POST', '/fleet/provision?count=2', OPERATOR);
    assert.equal(two.body.count, 2);
    assert.equal(new Set(two.body.keys).size, 2);
    const one = await as('POST', '/fleet/provision?count=0', OPERATOR);
    assert.equal(one.body.count, 1);
  });

  it('serves Prometheus metrics to the operator only', async () => {
    assert.equal((await as('GET', '/metrics')).status, 403);
    const res = await as('GET', '/metrics', OPERATOR);
    assert.match(res.headers['Content-Type'], /^text\/plain/);
    assert.equal(typeof res.body, 'string');
  });

  it('shows a key its own browsers, sessions and allowance in the fleet view', async () => {
    connectBrowser('b-fleet', KEY);
    const res = await as('GET', '/fleet');
    assert.equal(res.body.browsers.total, 1);
    assert.equal(res.body.sessions.total, 0);
    assert.ok(res.body.at);
    assert.ok('usage' in res.body);
  });

  it('does not show one key another key’s browsers', async () => {
    connectBrowser('b-fleet', 'someone-else');
    const res = await as('GET', '/fleet');
    assert.equal(res.body.browsers.total, 0);
  });

  it('reports a key’s usage with its connected browsers against the quota', async () => {
    connectBrowser('b-fleet', KEY);
    const res = await as('GET', '/usage');
    assert.equal(res.body.browsers.connected, 1);
    assert.ok(Array.isArray(res.body.history));
  });

  it('returns only the caller’s own audit trail', async () => {
    audit({ action: 'test.mine', actorKey: KEY });
    audit({ action: 'test.theirs', actorKey: 'someone-else' });
    const res = await as('GET', `/audit?limit=${MAX_AUDIT_LIMIT * 10}`);
    const actions = res.body.events.map((e) => e.action);
    assert.ok(actions.includes('test.mine'));
    assert.ok(!actions.includes('test.theirs'));
    assert.ok(res.body.events.every((e) => e.actor === fingerprint(KEY)));
  });

  it('drains and undrains for the operator', async () => {
    assert.equal((await as('POST', '/operator/drain', KEY, {})).status, 403);
    const on = await as('POST', '/operator/drain', OPERATOR, {});
    assert.equal(on.body.draining, true);
    assert.equal(registry.draining, true);
    const off = await as('POST', '/operator/drain', OPERATOR, { draining: false });
    assert.equal(off.body.draining, false);
    assert.equal(registry.draining, false);
  });
});
