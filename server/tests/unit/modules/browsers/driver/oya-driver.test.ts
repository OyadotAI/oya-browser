/**
 * Unit tests for the Oya browser's driver: commands, live-view control and
 * relayed CDP all travel over the browser's own control socket.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { OyaDriver } from '../../../../../src/modules/browsers/driver/oya-driver.ts';
import { LIVE_VIEW_FPS } from '../../../../../src/modules/browsers/connection/constants.ts';
import { onBrowserMessage } from '../../../../../src/modules/browsers/cdp-relay.ts';
import { FakeSocket } from '../../../support/fakes.ts';

describe('OyaDriver', () => {
  it('lists what the app announced, or the Oya list for an app too old to announce', () => {
    assert.deepEqual(new OyaDriver({ ws: new FakeSocket(), actions: ['click', 'wait'] }, 'b-oya').actions(), [
      'click',
      'wait',
    ]);
    const fallback = new OyaDriver({ ws: new FakeSocket() }, 'b-oya').actions();
    assert.ok(fallback.includes('workflow') && !fallback.includes('cookies'));
  });

  it('is a heartbeat client that is alive for as long as it is registered', () => {
    const driver = new OyaDriver({ ws: new FakeSocket() }, 'b-oya');
    assert.deepEqual([driver.kind, driver.heartbeat, driver.isAlive()], ['oya', true, true]);
  });

  it('asks the browser to push frames, and to stop, over its socket', async () => {
    const ws = new FakeSocket();
    const driver = new OyaDriver({ ws }, 'b-oya');
    await driver.startScreencast();
    await driver.stopScreencast();
    assert.deepEqual(ws.sent, [{ type: 'stream_start', fps: LIVE_VIEW_FPS }, { type: 'stream_stop' }]);
  });

  it('does not fail a viewer because the socket just went away', async () => {
    const ws = new FakeSocket();
    ws.failWith = new Error('socket closed');
    await new OyaDriver({ ws }, 'b-oya').startScreencast();
  });

  it('has no cookies to pull: the browser syncs its own jar', async () => {
    assert.equal(await new OyaDriver({ ws: new FakeSocket() }, 'b-oya').cookies(), null);
  });

  it('offers no CDP endpoint unless the browser said it relays CDP', () => {
    assert.equal(new OyaDriver({ ws: new FakeSocket() }, 'b-oya').cdpEndpoint(), null);
    assert.equal(new OyaDriver({ ws: new FakeSocket(), cdp: false }, 'b-oya').cdpEndpoint(), null);
  });

  it('opens CDP as a relay over the control socket once the browser confirms it', async () => {
    const ws = new FakeSocket();
    const endpoint = new OyaDriver({ ws, cdp: true }, 'b-relay').cdpEndpoint();
    assert.equal(endpoint.url, undefined);
    const opening = endpoint.open();
    const [{ sid }] = ws.ofType('cdp_open');
    onBrowserMessage('b-relay', { type: 'cdp_opened', sid });
    const relay = await opening;
    relay.send('{"id":1}');
    assert.deepEqual(ws.ofType('cdp'), [{ type: 'cdp', data: '{"id":1}', sid }]);
    relay.close();
  });

  it('closing it does nothing: the browser owns the socket', () => {
    const ws = new FakeSocket();
    new OyaDriver({ ws }, 'b-oya').close();
    assert.equal(ws.closed, null);
  });
});
