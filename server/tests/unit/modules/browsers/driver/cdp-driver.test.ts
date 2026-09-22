/**
 * Unit tests for the CDP browser's driver: each member of the port is the
 * engine's own, and nothing reaches past the engine's public surface.
 */
import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { CdpDriver } from '../../../../../src/modules/browsers/driver/cdp-driver.ts';

/** An engine stand-in with the members the driver uses. */
function engine(over: object = {}): any {
  const made = {
    alive: true,
    closed: false,
    wsUrl: 'ws://127.0.0.1:1/devtools/browser/x',
    isAlive: () => made.alive,
    startScreencast: mock.fn(async () => {}),
    stopScreencast: mock.fn(async () => {}),
    cookies: async () => [{ name: 'sid' }],
    close: () => (made.closed = true),
    ...over,
  };
  return made;
}

describe('CdpDriver', () => {
  it('lists the CDP actions, which every vendor shares', () => {
    const listed = new CdpDriver({ wsUrl: 'ws://x' } as any).actions();
    assert.ok(listed.includes('back') && listed.includes('cookies') && !listed.includes('workflow'));
  });

  it('is driven, not heard from: no heartbeat, and as alive as its engine says', () => {
    const e = engine();
    const driver = new CdpDriver(e);
    assert.deepEqual([driver.kind, driver.heartbeat, driver.isAlive()], ['cdp', false, true]);
    e.alive = false;
    assert.equal(driver.isAlive(), false);
  });

  it('takes an engine that cannot say whether it is alive to be alive', () => {
    assert.equal(new CdpDriver(engine({ isAlive: undefined })).isAlive(), true);
  });

  it('hands the viewer’s frame callback to the engine’s screencast, and stops it', async () => {
    const e = engine();
    const onFrame = () => {};
    const driver = new CdpDriver(e);
    await driver.startScreencast(onFrame);
    await driver.stopScreencast();
    assert.deepEqual(e.startScreencast.mock.calls[0].arguments, [onFrame]);
    assert.equal(e.stopScreencast.mock.callCount(), 1);
  });

  it('reads the browser’s cookies through the engine', async () => {
    assert.deepEqual(await new CdpDriver(engine()).cookies(), [{ name: 'sid' }]);
  });

  it('offers the vendor’s address as its CDP endpoint, and none when there is no address', () => {
    assert.equal(new CdpDriver(engine()).cdpEndpoint().url, 'ws://127.0.0.1:1/devtools/browser/x');
    assert.equal(new CdpDriver(engine({ wsUrl: undefined })).cdpEndpoint(), null);
  });

  it('closes the engine’s connection', () => {
    const e = engine();
    new CdpDriver(e).close();
    assert.equal(e.closed, true);
  });
});
