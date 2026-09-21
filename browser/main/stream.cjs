/** The live view: frames of the active tab over the control socket while someone watches. */
const WebSocket = require('ws');
const { STREAM_MIN_FRAME_MS, MS_PER_SECOND, STREAM_MAX_BUFFERED, STREAM_JPEG_QUALITY } = require('./constants.cjs');

/** Whether a frame should be taken now: someone is connected, the socket keeps up, none is in flight. */
function streamCanCapture(stream, view, ws) {
  return (
    ws && ws.readyState === WebSocket.OPEN && view && !stream.capturing && ws.bufferedAmount <= STREAM_MAX_BUFFERED
  );
}

/**
 * capturePage() returns device pixels, 2x the page on a retina screen. The
 * live view maps a click through the frame's own width and sends it as CSS
 * pixels, so an unscaled frame puts every click at twice the distance from
 * the top-left: near enough at the corner, nowhere near the target at the
 * other edge. Send the page at the size the page thinks it is.
 */
function streamFrameAtPageSize(view, img) {
  const { width, height } = view.getBounds();
  return width > 0 && img.getSize().width !== width ? img.resize({ width, height, quality: 'good' }) : img;
}

/** Captures and sends one frame, if the socket can take it. */
async function streamTick(stream) {
  const view = stream.activeView();
  if (!streamCanCapture(stream, view, stream.socket())) return;
  stream.capturing = true;
  await streamSendFrame(stream, view).catch(() => {});
  stream.capturing = false;
}

/** Captures the view and sends it as a JPEG frame. */
async function streamSendFrame(stream, view) {
  const frame = streamFrameAtPageSize(view, await view.webContents.capturePage());
  const data = 'data:image/jpeg;base64,' + frame.toJPEG(STREAM_JPEG_QUALITY).toString('base64');
  stream.send({ type: 'frame', data });
}

/** Stops the frame loop, if one runs. */
function streamStop(stream) {
  if (stream.interval) {
    clearInterval(stream.interval);
    stream.interval = null;
  }
}

/** Starts a frame loop at `fps`, replacing any running one. */
function streamStart(stream, fps) {
  streamStop(stream);
  const ms = Math.max(STREAM_MIN_FRAME_MS, Math.round(MS_PER_SECOND / fps));
  stream.interval = setInterval(() => streamTick(stream), ms);
}

/** `activeView()` is the tab to show, `socket()` the control socket, `send` writes to it. */
function createStream({ activeView, socket, send }) {
  const stream = { activeView, socket, send, interval: null, capturing: false };
  return { startStream: (fps) => streamStart(stream, fps), stopStream: () => streamStop(stream) };
}

module.exports = { createStream };
