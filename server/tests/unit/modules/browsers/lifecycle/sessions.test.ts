/**
 * Unit tests for hosted CDP sessions: a raw CDP URL is checked by the network
 * guard, the vendor's cleanup is recorded on the durable session, a failed
 * release is logged, and a driven browser is registered and counted.
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
      await assert.rejects(acquireSession(req, 'key-a', 'cdp', 'ws://127.0.0.1:9222'), { status: 400 });
    } finally {
      if (saved !== undefined) process.env.OYA_ALLOW_PRIVATE_TARGETS = saved;
    }
  });

  it('acquires a public CDP URL as a session with nothing to release', async () => {
    const req = fakeRequest({ extra: { controlSession: { id: B } } });
    const session = await acquireSession(req, 'key-a', 'cdp', 'wss://8.8.8.8/devtools');
    assert.equal(session.wsUrl, 'wss://8.8.8.8/devtools');
    assert.equal(session.provider, 'cdp');
    await session.release();
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
      { persona: { id: 'p' } },
    );
    const b = registry.get(B);
    assert.deepEqual([b.name, b.clientType, b.provider, b.persona.id], ['Agent', 'cdp', 'steel', 'p']);
    assert.equal(usage.current('key-a').browsers_started, 1);
  });
});
