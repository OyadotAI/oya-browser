/**
 * What a registered browser runs as: the operator's residential gateway reaches
 * only a cloud browser on a plan that includes proxy traffic, on every connect,
 * however many browsers ran as the same persona before it.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { identityFor } from '../../../../../src/modules/browsers/connection/identity.ts';
import { CLOUD_PROVIDER } from '../../../../../src/modules/browsers/connection/constants.ts';
import { container } from '../../../../../src/app/container.ts';

/** A gateway whose credentials must never leave a cloud browser. */
const GATEWAY = 'http://user-country-{geo}-session-{session}:operator-secret@gate.vendor.test:7000';

/** A provider that is not one of our cloud sandboxes. */
const DESKTOP = 'desktop';

/** A persona of its own for each test, so no test sees another's cached fingerprint. */
let next = 0;
const persona = (proxy = null) => ({ id: `p-identity-${++next}`, seed: next, prefs: {}, proxy });

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
});
