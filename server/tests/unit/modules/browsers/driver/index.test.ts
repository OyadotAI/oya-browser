/**
 * Unit tests for the one place a browser's driver is chosen, and for the
 * registry record that always carries one.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { driverFor } from '../../../../../src/modules/browsers/driver/index.ts';
import { registry } from '../../../../../src/modules/browsers/registry.ts';
import { FakeSocket, connectBrowser, disconnectBrowser } from '../../../support/fakes.ts';
import { driveBrowser } from '../../../support/browsers.ts';

describe('driverFor', () => {
  afterEach(() => ['b-pick-oya', 'b-pick-cdp'].forEach(disconnectBrowser));

  it('picks the CDP driver for a browser we dialled, and the Oya driver for one that dialled us', () => {
    assert.equal(driverFor({ engine: { wsUrl: 'ws://x' } as any }, 'b').kind, 'cdp');
    assert.equal(driverFor({ ws: new FakeSocket() }, 'b').kind, 'oya');
  });

  it('refuses a CDP browser registered without its engine, rather than driving it as an Oya browser', () => {
    assert.throws(() => driverFor({ clientType: 'cdp' }, 'b'), {
      message: 'A cdp browser needs the engine that drives it',
    });
  });

  it('every registered browser carries a driver, whichever way it connected', () => {
    connectBrowser('b-pick-oya');
    driveBrowser('b-pick-cdp', () => ({ ok: true }));
    assert.equal(registry.get('b-pick-oya').driver.kind, 'oya');
    assert.equal(registry.get('b-pick-cdp').driver.kind, 'cdp');
  });
});
