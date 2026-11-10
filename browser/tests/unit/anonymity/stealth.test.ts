/**
 * Unit tests for the passkey body of the page script (stealth.js): which WebAuthn
 * requests it answers as a dismissed dialog, and which reach the browser untouched.
 */
import { describe, it, mock, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { buildMaskPreamble, buildPasskeyBody } = require('../../../anonymity/stealth.js');

/** Longer than the slowest simulated dismissal. */
const DISMISS_WAIT_MS = 3000;

/** A page whose `navigator.credentials` records every request that reaches the browser. */
function page(options) {
  const reached = [];
  /** Stands in for the browser's own container; the page script patches its prototype. */
  class CredentialsContainer {
    /** A sign-in request that reached the browser. */
    get(o) {
      reached.push(['get', o]);
      return Promise.resolve('native');
    }
    /** A create request that reached the browser. */
    create(o) {
      reached.push(['create', o]);
      return Promise.resolve('native');
    }
  }
  const window = { CredentialsContainer, DOMException, setTimeout: (...a) => setTimeout(...a) };
  window.window = window;
  vm.createContext(window);
  const body = `(function(){'use strict';${buildMaskPreamble()}${buildPasskeyBody(options)}})();`;
  vm.runInContext(body, window);
  return { credentials: new CredentialsContainer(), reached };
}

/** Whether the request settled natively (true) or as a dismissed dialog (false). */
async function reachesBrowser(credentials, method, publicKey) {
  const result = credentials[method]({ publicKey });
  mock.timers.tick(DISMISS_WAIT_MS);
  try {
    return (await result) === 'native';
  } catch (err) {
    assert.equal(err.name, 'NotAllowedError');
    return false;
  }
}

const SECURITY_KEY = { authenticatorSelection: { authenticatorAttachment: 'cross-platform' } };
const THIS_MAC = { authenticatorSelection: { authenticatorAttachment: 'platform' } };

describe('passkey body', () => {
  beforeEach(() => mock.timers.enable({ apis: ['setTimeout'] }));
  afterEach(() => mock.timers.reset());

  it('dismisses every modal request when the runtime has no passkey dialog at all', async () => {
    const { credentials } = page();
    assert.equal(await reachesBrowser(credentials, 'create', THIS_MAC), false);
    assert.equal(await reachesBrowser(credentials, 'get', {}), false);
  });

  it('roaming only: dismisses creating a security-key or phone credential', async () => {
    const { credentials, reached } = page({ roamingOnly: true });
    assert.equal(await reachesBrowser(credentials, 'create', SECURITY_KEY), false);
    assert.deepEqual(reached, []);
  });

  it('roaming only: creating a passkey on this Mac, or with no attachment asked, reaches the browser', async () => {
    const { credentials } = page({ roamingOnly: true });
    assert.equal(await reachesBrowser(credentials, 'create', THIS_MAC), true);
    assert.equal(await reachesBrowser(credentials, 'create', {}), true);
  });

  it('roaming only: dismisses a sign-in only a security key or phone can answer', async () => {
    const { credentials } = page({ roamingOnly: true });
    const allowCredentials = [{ transports: ['usb', 'nfc'] }, { transports: ['hybrid'] }];
    assert.equal(await reachesBrowser(credentials, 'get', { allowCredentials }), false);
  });

  it('roaming only: a sign-in this Mac might answer reaches the browser', async () => {
    const { credentials } = page({ roamingOnly: true });
    for (const allowCredentials of [
      undefined,
      [],
      [{ transports: ['internal', 'hybrid'] }],
      [{ id: 'unknown' }],
      [null],
    ])
      assert.equal(
        await reachesBrowser(credentials, 'get', { allowCredentials }),
        true,
        JSON.stringify(allowCredentials),
      );
  });

  it('leaves passkey autofill pending natively', async () => {
    const { credentials, reached } = page({ roamingOnly: true });
    credentials.get({ publicKey: { allowCredentials: [{ transports: ['usb'] }] }, mediation: 'conditional' });
    assert.equal(reached.length, 1);
  });
});
