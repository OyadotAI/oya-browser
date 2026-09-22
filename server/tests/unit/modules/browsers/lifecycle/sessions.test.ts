/**
 * Unit tests for hosted CDP sessions: a raw CDP URL is checked by the network
 * guard, an endpoint one browser drives is refused to a second, the vendor's
 * cleanup is recorded on the durable session, a failed release is logged, and
 * a driven browser is registered and counted.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import {
  acquireSession,
  addCdpBrowser,
  claimSession,
  releaseQuietly,
} from '../../../../../src/modules/browsers/lifecycle/sessions.ts';
import { registry } from '../../../../../src/modules/browsers/registry.ts';
import * as usage from '../../../../../src/platform/usage.ts';
import { disconnectBrowser } from '../../../support/fakes.ts';
import { fakeRequest, stubControl } from '../../../support/browsers.ts';

const B = 'b-sessions';

describe('sessions', () => {
  afterEach(() => {
    mock.restoreAll();
    disconnectBrowser(B);
  });

  it('refuses a raw CDP URL that points inside the network', async () => {
    const saved = process.env.OYA_ALLOW_PRIVATE_TARGETS;
    delete process.env.OYA_ALLOW_PRIVATE_TARGETS;
    try {
      const req = fakeRequest({ extra: { controlSession: { id: B } } });
      await assert.rejects(acquireSession(req, 'key-a', 'cdp', 'ws://127.0.0.1:9222', B), { status: 400 });
    } finally {
      if (saved !== undefined) process.env.OYA_ALLOW_PRIVATE_TARGETS = saved;
    }
  });

  it('acquires a public CDP URL as a session with nothing to release', async () => {
    const req = fakeRequest({ extra: { controlSession: { id: B } } });
    const session = await acquireSession(req, 'key-a', 'cdp', 'wss://8.8.8.8/devtools', B);
    assert.equal(session.wsUrl, 'wss://8.8.8.8/devtools');
    assert.equal(session.provider, 'cdp');
    await session.release();
  });

  /** Acquires `wsUrl` for `browserId` under `key`. */
  const acquireFor = (key: string, wsUrl: string, browserId: string) =>
    acquireSession(fakeRequest({ extra: { controlSession: { id: browserId } } }), key, 'cdp', wsUrl, browserId);

  it('refuses a second session on an endpoint the same key already drives, naming the holder', async () => {
    const first = await acquireFor('key-a', 'wss://8.8.8.8/devtools/browser/abc', B);
    try {
      await assert.rejects(acquireFor('key-a', 'wss://8.8.8.8/devtools/browser/abc', 'b-second'), {
        status: 409,
        code: 'endpoint_in_use',
        browserId: B,
        message: `The Chrome at 8.8.8.8 is already held by browser ${B}. Stop that browser, or drive it.`,
      });
    } finally {
      await first.release();
    }
  });

  it('refuses across keys without naming the holder', async () => {
    const first = await acquireFor('key-a', 'wss://8.8.8.8:8443/devtools', B);
    try {
      const err = await acquireFor('key-b', 'wss://8.8.8.8:8443/devtools', 'b-other').catch((e) => e);
      assert.deepEqual([err.status, err.code, err.browserId], [409, 'endpoint_in_use', undefined]);
      assert.equal(
        err.message,
        'The Chrome at 8.8.8.8:8443 is already held by another browser. Stop it there, or start on another endpoint.',
      );
    } finally {
      await first.release();
    }
  });

  it('treats one endpoint written two ways as one', async () => {
    const first = await acquireFor('key-a', 'WSS://8.8.8.8:443/devtools', B);
    try {
      await assert.rejects(acquireFor('key-a', 'wss://8.8.8.8/devtools', 'b-second'), { status: 409 });
    } finally {
      await first.release();
    }
  });

  it('treats one Chrome reached by two host names as one, by the browser id in its address', async () => {
    const id = '0a1b2c3d-4e5f-6a7b-8c9d-0e1f2a3b4c5d';
    const first = await acquireFor('key-a', `wss://8.8.8.8:9222/devtools/browser/${id}`, B);
    try {
      await assert.rejects(acquireFor('key-a', `wss://8.8.4.4:9222/devtools/browser/${id}`, 'b-second'), {
        status: 409,
        browserId: B,
      });
    } finally {
      await first.release();
    }
  });

  it('frees the endpoint once the session is released', async () => {
    const first = await acquireFor('key-a', 'wss://8.8.8.8/devtools', B);
    await first.release();
    const second = await acquireFor('key-a', 'wss://8.8.8.8/devtools', 'b-second');
    assert.equal(second.wsUrl, 'wss://8.8.8.8/devtools');
    await second.release();
  });

  it('hands a refused duplicate\u2019s vendor session back', async () => {
    const first = await acquireFor('key-a', 'wss://8.8.8.8/devtools', B);
    // Browser Use over a faked network: its session is created, then must be stopped.
    const calls: string[] = [];
    mock.method(globalThis, 'fetch', async (url: string) => {
      calls.push(String(url));
      return Response.json({ id: 's-dup', cdpUrl: 'wss://8.8.8.8/devtools' });
    });
    process.env.BROWSERUSE_API_KEY = 'k';
    stubControl();
    try {
      const req = fakeRequest({ extra: { controlSession: { id: 'b-second' } } });
      await assert.rejects(acquireSession(req, 'key-a', 'browseruse', undefined, 'b-second'), { status: 409 });
      assert.equal(calls.filter((u) => u.endsWith('/browsers/s-dup')).length, 1);
    } finally {
      delete process.env.BROWSERUSE_API_KEY;
      await first.release();
    }
  });

  it('records the vendor’s cleanup before checking provisioning may go on', async () => {
    const { update, assertProvisioning } = stubControl();
    await claimSession('key-a', B, { cleanup: { kind: 'steel', id: 's1' } });
    assert.deepEqual(update.mock.calls[0].arguments, ['key-a', B, { cleanup: { kind: 'steel', id: 's1' } }]);
    assert.equal(assertProvisioning.mock.callCount(), 1);
  });

  it('skips the cleanup record for a session that has none', async () => {
    const { update } = stubControl();
    await claimSession('key-a', B, {});
    assert.equal(update.mock.callCount(), 0);
  });

  it('logs a release that fails instead of throwing', async () => {
    const logged = mock.method(console, 'error', () => {});
    await releaseQuietly({ release: async () => Promise.reject(new Error('vendor 500')) });
    assert.match(String(logged.mock.calls[0].arguments[1]), /vendor 500/);
  });

  it('registers a CDP browser under its display name and counts it as started', () => {
    usage.reset();
    addCdpBrowser(
      fakeRequest({ body: { name: 'Agent' } }),
      'key-a',
      B,
      { provider: 'steel' },
      { persona: { id: 'p' }, engine: { close() {} } as any },
    );
    const b = registry.get(B);
    assert.deepEqual([b.name, b.clientType, b.provider, b.persona.id], ['Agent', 'cdp', 'steel', 'p']);
    assert.equal(usage.current('key-a').browsers_started, 1);
  });
});
