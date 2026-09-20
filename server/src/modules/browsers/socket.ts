/**
 * The browsers' control sockets: accepting a connection, and sending commands
 * to connected browsers. The work lives in connection/; this is the entry point
 * the rest of the server imports.
 */
import { registry } from './registry.ts';
import { BrowserConnection } from './connection/browser-connection.ts';
import { LIVE_VIEW_FPS } from './connection/constants.ts';

export { sendCommand, takeDialogNote } from './connection/commands.ts';

/** Takes over a new browser socket; it authenticates within 10s or is closed. */
export function handleConnection(ws, req?) {
  new BrowserConnection(ws, req);
}

/** Tells a browser to start or stop sending live-view frames. */
function streamControl(message: object) {
  return ({ id }) => {
    try {
      registry.get(id)?.ws?.send(JSON.stringify(message));
    } catch {}
  };
}

registry.on('stream:start', streamControl({ type: 'stream_start', fps: LIVE_VIEW_FPS }));
registry.on('stream:stop', streamControl({ type: 'stream_stop' }));
