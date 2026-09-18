/**
 * Unit tests for the browsers' socket entry point: a live-view viewer arriving
 * or leaving tells an Oya browser to start or stop sending frames.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import '../../../../src/modules/browsers/socket.ts';
import { registry } from '../../../../src/modules/browsers/registry.ts';
import { LIVE_VIEW_FPS } from '../../../../src/modules/browsers/connection/constants.ts';
import { connectBrowser, disconnectBrowser } from '../../support/fakes.ts';
import { FakeResponse } from '../../support/browsers.ts';

const B = 'b-socket';

describe('socket: live-view control', () => {
  afterEach(() => disconnectBrowser(B));

  it('asks the browser for frames at the live-view rate when the first viewer arrives', () => {
    const ws = connectBrowser(B);
    registry.addViewer(B, new FakeResponse());
    assert.deepEqual(ws.ofType('stream_start'), [{ type: 'stream_start', fps: LIVE_VIEW_FPS }]);
  });

  it('tells the browser to stop once the last viewer leaves', () => {
    const ws = connectBrowser(B);
    const viewer = new FakeResponse();
    registry.addViewer(B, viewer);
    registry.removeViewer(B, viewer);
    assert.equal(ws.ofType('stream_stop').length, 1);
  });

  it('shrugs off a socket that cannot be written', () => {
    const ws = connectBrowser(B);
    ws.failWith = new Error('closed');
    assert.doesNotThrow(() => registry.addViewer(B, new FakeResponse()));
  });
});
