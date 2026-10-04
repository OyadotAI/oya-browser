/**
 * Unit tests for LiveStream (src/main/connection/stream.ts): frames of the active tab at the page's own
 * size, paced, and skipped while the socket is behind or down.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import WebSocket from 'ws';
import { LiveStream } from '../../../../src/main/connection/stream.ts';
import { flush } from '../../support/fakes.cjs';
import { STREAM_MIN_FRAME_MS, STREAM_MAX_BUFFERED } from '../../../../src/main/connection/constants.ts';

/** A captured image of `width` device pixels that records resizes. */
function fakeImage(width) {
  const image = {
    resized: null,
    getSize: () => ({ width }),
    resize: (size) => ({ ...image, resized: size, toJPEG: () => Buffer.from('small') }),
    toJPEG: () => Buffer.from('full'),
  };
  return image;
}

/** A view of `width`×100 CSS pixels whose capture is `image`. */
const fakeView = (width, image) => ({
  getBounds: () => ({ width, height: 100 }),
  webContents: { capturePage: async () => image },
});

describe('LiveStream', () => {
  let sent: any[];
  let socket: any;
  beforeEach(() => {
    mock.timers.enable({ apis: ['setInterval'] });
    sent = [];
    socket = { readyState: WebSocket.OPEN, bufferedAmount: 0 };
  });
  afterEach(() => mock.timers.reset());

  /** A stream over `view` and the shared fake socket. */
  const streamOf = (view) => {
    const live = new LiveStream({
      tabs: { getActiveView: () => view },
      socket: {
        get ws() {
          return socket;
        },
        send: (m) => sent.push(m),
      },
    });
    return { startStream: (fps: number) => live.startStream(fps), stopStream: () => live.stopStream() };
  };

  it('sends a JPEG frame of the active tab every interval', async () => {
    const { startStream, stopStream } = streamOf(fakeView(800, fakeImage(800)));
    startStream(2);
    mock.timers.tick(500);
    await flush();
    assert.deepEqual(sent, [
      { type: 'frame', data: 'data:image/jpeg;base64,' + Buffer.from('full').toString('base64') },
    ]);
    stopStream();
  });

  it('scales a device-pixel capture down to the page size', async () => {
    const { startStream, stopStream } = streamOf(fakeView(800, fakeImage(1600)));
    startStream(2);
    mock.timers.tick(500);
    await flush();
    assert.equal(sent[0].data, 'data:image/jpeg;base64,' + Buffer.from('small').toString('base64'));
    stopStream();
  });

  it('never streams faster than the minimum frame interval', async () => {
    const { startStream, stopStream } = streamOf(fakeView(800, fakeImage(800)));
    startStream(100);
    mock.timers.tick(STREAM_MIN_FRAME_MS - 1);
    await flush();
    assert.equal(sent.length, 0);
    mock.timers.tick(1);
    await flush();
    assert.equal(sent.length, 1);
    stopStream();
  });

  it('skips frames while the socket is behind or closed', async () => {
    const { startStream, stopStream } = streamOf(fakeView(800, fakeImage(800)));
    startStream(5);
    socket.bufferedAmount = STREAM_MAX_BUFFERED + 1;
    mock.timers.tick(200);
    socket.bufferedAmount = 0;
    socket.readyState = WebSocket.CLOSED;
    mock.timers.tick(200);
    await flush();
    assert.equal(sent.length, 0);
    stopStream();
  });

  it('keeps streaming after a capture fails', async () => {
    let fail = true;
    const view = fakeView(800, fakeImage(800));
    const capture = view.webContents.capturePage;
    view.webContents.capturePage = async () => {
      if (fail) throw new Error('crashed');
      return capture();
    };
    const { startStream, stopStream } = streamOf(view);
    startStream(5);
    mock.timers.tick(200);
    await flush();
    fail = false;
    mock.timers.tick(200);
    await flush();
    assert.equal(sent.length, 1);
    stopStream();
  });

  it('stops sending once stopped', async () => {
    const { startStream, stopStream } = streamOf(fakeView(800, fakeImage(800)));
    startStream(5);
    stopStream();
    mock.timers.tick(1000);
    await flush();
    assert.equal(sent.length, 0);
  });
});
