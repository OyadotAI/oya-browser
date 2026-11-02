/**
 * What a registered browser runs as: the operator's residential gateway reaches
 * only a cloud browser on a plan that includes proxy traffic, on every connect,
 * however many browsers ran as the same persona before it; and a persona that
 * asked for no proxy (`proxy.direct`) gets none at all.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { identityFor } from '../../../../../src/modules/browsers/connection/identity.ts';
import { CLOUD_PROVIDER } from '../../../../../src/modules/browsers/connection/constants.ts';
import { container } from '../../../../../src/app/container.ts';
import * as proxies from '../../../../../src/modules/proxies/service.ts';
import { proxies as pool } from '../../../../../src/modules/proxies/store.ts';
import { Proxy } from '../../../../../src/modules/proxies/proxy.ts';

/** A gateway whose credentials must never leave a cloud browser. */
const GATEWAY = 'http://user-country-{geo}-session-{session}:operator-secret@gate.vendor.test:7000';

/** A provider that is not one of our cloud sandboxes. */
const DESKTOP = 'desktop';

/** A persona of its own for each test, so no test sees another's cached fingerprint. */
let next = 0;
const persona = (proxy: Record<string, unknown> | null = null) => ({
  id: `p-identity-${++next}`,
  seed: next,
  prefs: {},
  proxy,
});

/** A persona that asked to run with no proxy. */
const direct = () => persona({ direct: true });

/** A shared, healthy proxy with room for one persona, put straight into the pool. */
function sharedProxy(id: string) {
  const p = new Proxy({ id, sealed: 'x', owner: null });
  pool.set(id, p);
  return p;
}

/** Whether this key's plan includes proxy traffic, per call. */
let allowed = true;

describe('identityFor', () => {
  const saved = process.env.OYA_RESIDENTIAL_PROXY_URL;

  beforeEach(() => {
    process.env.OYA_RESIDENTIAL_PROXY_URL = GATEWAY;
    allowed = true;
    mock.method(container.billing.entitlements, 'proxyAllowed', () => allowed);
    mock.method(console, 'warn', () => {});
    mock.method(console, 'log', () => {});
  });

  afterEach(() => {
    mock.restoreAll();
    proxies.reset();
    if (saved === undefined) delete process.env.OYA_RESIDENTIAL_PROXY_URL;
    else process.env.OYA_RESIDENTIAL_PROXY_URL = saved;
  });

  it('gives a desktop that connects after a cloud browser no residential credentials', () => {
    const p = persona();
    assert.equal(identityFor('k-identity', p, CLOUD_PROVIDER).residential, true);
    const desktop = identityFor('k-identity', p, DESKTOP);
    assert.equal(desktop.fingerprint.proxy, null);
    assert.equal(desktop.residential, false);
  });

  it('gives a plan without proxy traffic no residential credentials after a paid run', () => {
    const p = persona();
    identityFor('k-identity', p, CLOUD_PROVIDER);
    allowed = false;
    const free = identityFor('k-identity', p, CLOUD_PROVIDER);
    assert.equal(free.fingerprint.proxy, null);
    assert.equal(free.residential, false);
  });

  it('meters every cloud connect, not only the first', () => {
    const p = persona();
    const runs = [identityFor('k-identity', p, CLOUD_PROVIDER), identityFor('k-identity', p, CLOUD_PROVIDER)];
    assert.deepEqual(
      runs.map((r) => [r.residential, r.fingerprint.proxy.metered]),
      [
        [true, true],
        [true, true],
      ],
    );
  });

  it("carries the persona's proxy as it is now, after an edit", () => {
    const p = persona({ host: 'old.proxy.test', port: 1 });
    identityFor('k-identity', p, DESKTOP);
    p.proxy = { host: 'new.proxy.test', port: 2 };
    assert.deepEqual(identityFor('k-identity', p, DESKTOP).fingerprint.proxy, p.proxy);
  });

  it('gives a direct persona no proxy, even with the residential gateway configured and allowed', () => {
    const run = identityFor('k-identity', direct(), CLOUD_PROVIDER);
    assert.equal(run.fingerprint.proxy, null);
    assert.equal(run.residential, false);
  });

  it('still falls back to the residential gateway for a persona that is not direct', () => {
    const run = identityFor('k-identity', persona({ geo: 'US' }), CLOUD_PROVIDER);
    assert.equal(run.residential, true);
    assert.equal(run.fingerprint.proxy.metered, true);
  });

  it('takes no proxy slot for a direct persona', () => {
    const shared = sharedProxy('px-identity-free');
    const p = direct();
    identityFor('k-identity', p, CLOUD_PROVIDER);
    assert.equal(proxies.assigned(p.id), null);
    assert.equal(shared.assigned, 0);
  });

  it('gives back the slot a persona held before it became direct', () => {
    const shared = sharedProxy('px-identity-held');
    const p = direct();
    proxies.assign('k-identity', p.id, shared.id);
    identityFor('k-identity', p, CLOUD_PROVIDER);
    assert.equal(proxies.assigned(p.id), null);
  });
});
