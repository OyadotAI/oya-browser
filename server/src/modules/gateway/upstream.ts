/**
 * Dialling a browser's CDP WebSocket.
 */
import { WebSocket } from 'ws';
import { MAX_PAYLOAD_BYTES, UPSTREAM_CONNECT_MS } from './constants.ts';

/** A new socket to the browser's CDP endpoint. */
export const dial = (wsUrl) =>
  new WebSocket(wsUrl, { maxPayload: MAX_PAYLOAD_BYTES, handshakeTimeout: UPSTREAM_CONNECT_MS });

/** Resolves once the socket opens; rejects on its first error. */
export function opened(ws) {
  return new Promise((resolve, reject) => {
    ws.once('open', resolve);
    ws.once('error', reject);
  });
}

/** Stops the timer, then settles. */
const settle = (timer, fn, value?) => {
  clearTimeout(timer);
  fn(value);
};

/** Dials the browser and waits for it to open, giving up after UPSTREAM_CONNECT_MS. */
export async function dialWithTimeout(wsUrl) {
  const upstream = dial(wsUrl);
  await new Promise((resolve, reject) => {
    const t = setTimeout(() => reject(new Error('upstream connect timed out')), UPSTREAM_CONNECT_MS);
    upstream.once('open', () => settle(t, resolve, undefined));
    upstream.once('error', (e) => settle(t, reject, e));
  });
  return upstream;
}
