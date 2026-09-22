/**
 * Unit tests for live view: a viewer arriving or leaving starts or stops a
 * browser's frames through its driver, whichever kind of browser it is.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import '../../../../src/modules/browsers/live-view.ts';
import { registry } from '../../../../src/modules/browsers/registry.ts';
import { LIVE_VIEW_FPS } from '../../../../src/modules/browsers/connection/constants.ts';
import { connectBrowser, disconnectBrowser } from '../../support/fakes.ts';
import { FakeResponse, driveBrowser } from '../../support/browsers.ts';

const B = 'b-live-view';

describe('live view', () => {
  afterEach(() => disconnectBrowser(B));

  it('asks an Oya browser for frames at the live-view rate when the first viewer arrives', () => {
    const ws = connectBrowser(B);
    registry.addViewer(B, new FakeResponse());
    assert.deepEqual(ws.ofType('stream_start'), [{ type: 'stream_start', fps: LIVE_VIEW_FPS }]);
  });

  it('tells an Oya browser to stop once the last viewer leaves', () => {
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

  it('starts a CDP browser’s screencast for its first viewer, pushes its frames, and stops it after the last', async () => {
    const engine: any = driveBrowser(B, () => ({ ok: true }));
    let onFrame;
    engine.startScreencast = mock.fn(async (cb) => (onFrame = cb));
    engine.stopScreencast = mock.fn(async () => {});
    const viewer = new FakeResponse();
    registry.addViewer(B, viewer);
    onFrame('frame-1');
    assert.equal(registry.get(B).lastFrame, 'frame-1');
    registry.removeViewer(B, viewer);
    assert.equal(engine.stopScreencast.mock.callCount(), 1);
  });

  it('a screencast that fails to start does not take the viewer down', async () => {
    const engine: any = driveBrowser(B, () => ({ ok: true }));
    engine.startScreencast = async () => Promise.reject(new Error('target closed'));
    assert.doesNotThrow(() => registry.addViewer(B, new FakeResponse()));
    await new Promise((resolve) => setImmediate(resolve));
  });

  it('a viewer event for a browser that is gone does nothing', () => {
    assert.doesNotThrow(() => registry.emit('stream:start', { id: 'b-gone' }));
    assert.doesNotThrow(() => registry.emit('stream:stop', { id: 'b-gone' }));
  });
});
