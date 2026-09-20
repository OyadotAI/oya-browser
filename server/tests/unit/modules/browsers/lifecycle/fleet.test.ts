/**
 * Unit tests for the fleet helpers starting and stopping share: the connected
 * gauge, dropping and forgetting browsers, the per-key browser quota and the
 * name a CDP browser is listed under.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import {
  browserQuota,
  countBrowsers,
  displayName,
  dropBrowser,
  forget,
  quotaBody,
} from '../../../../../src/modules/browsers/lifecycle/fleet.ts';
import { registry } from '../../../../../src/modules/browsers/registry.ts';
import { metrics, snapshot } from '../../../../../src/platform/metrics.ts';
import { QUOTAS } from '../../../../../src/platform/limits.ts';
import { MAX_NAME, OPERATOR_CLOSE } from '../../../../../src/modules/browsers/constants.ts';
import { connectBrowser, disconnectBrowser } from '../../../support/fakes.ts';

const B = 'b-fleet';

describe('fleet', () => {
  afterEach(() => disconnectBrowser(B));

  it('publishes how many browsers this replica holds', () => {
    connectBrowser(B);
    countBrowsers();
    assert.equal(snapshot().oya_browsers_connected[0].value, registry.browsers.size);
    metrics.browsersConnected.set({}, 0);
  });

  it('closes a dropped browser’s socket with the operator code and forgets it', () => {
    const ws = connectBrowser(B);
    dropBrowser(B, ws, 'Stopped by operator');
    assert.deepEqual(ws.closed, { code: OPERATOR_CLOSE, reason: 'Stopped by operator' });
    assert.equal(registry.isConnected(B), false);
  });

  it('drops a browser whose socket will not close', () => {
    const ws = connectBrowser(B);
    ws.close = () => {
      throw new Error('already closed');
    };
    dropBrowser(B, ws, 'x');
    assert.equal(registry.isConnected(B), false);
  });

  it('forgets a browser whose driver closed, and ignores one already gone', () => {
    connectBrowser(B);
    forget(B);
    assert.equal(registry.isConnected(B), false);
    assert.doesNotThrow(() => forget(B));
  });

  it('counts the key’s own browsers against its quota', () => {
    connectBrowser(B, 'fleet-key');
    const quota = browserQuota('fleet-key');
    assert.deepEqual(quota, { allowed: 1 < QUOTAS.browsers, quota: QUOTAS.browsers, current: 1 });
  });

  it('refuses once the key holds its quota', () => {
    const saved = QUOTAS.browsers;
    QUOTAS.browsers = 1;
    try {
      connectBrowser(B, 'fleet-full');
      assert.equal(browserQuota('fleet-full').allowed, false);
      assert.equal(quotaBody(browserQuota('fleet-full')).error, 'Browser quota reached (1)');
    } finally {
      QUOTAS.browsers = saved;
    }
  });

  it('lists a CDP browser under the caller’s name, else the vendor’s, cut to length', () => {
    assert.equal(displayName({ body: { name: 'Mine' } }, { provider: 'steel' }), 'Mine');
    assert.equal(displayName({}, { provider: 'steel' }), 'steel browser');
    assert.equal(displayName({ body: { name: 'n'.repeat(500) } }, {}).length, MAX_NAME);
  });
});
