/**
 * The browsers' control sockets: accepting a connection, and sending commands
 * to connected browsers. The work lives in connection/; this is the entry point
 * the rest of the server imports.
 */
import { BrowserConnection } from './connection/browser-connection.ts';
import './live-view.ts';
import { registry } from './registry.ts';

export { sendCommand, takeDialogNote } from './connection/commands.ts';
export { QUIET } from './connection/reporter.ts';
export { releaseBrowser } from './connection/release.ts';

/** The actions a connected browser does, or null when it is not connected here. */
export const actionsOf = (browserId: string): readonly string[] | null =>
  registry.get(browserId)?.driver.actions() ?? null;

/** Takes over a new browser socket; it authenticates within 10s or is closed. */
export function handleConnection(ws, req?) {
  new BrowserConnection(ws, req);
}

/**
 * Waits up to `ms` for browser `browserId` to be connected here under key `apiKey`; answers whether
 * it is. A desktop that drops its socket is back within a second or so, and a command sent in that
 * gap would fail for a browser that is about to be there. The key is checked because a browser id
 * is only its owner's while it is registered: a run that waited must never carry on in a browser
 * another key registered under the same id in the gap. `signal` ends the wait early (a Stop).
 */
export function whenConnected(browserId: string, apiKey: string, ms: number, signal?: AbortSignal): Promise<boolean> {
  const owned = () => registry.belongsTo(browserId, apiKey);
  if (registry.isConnected(browserId) || signal?.aborted) return Promise.resolve(owned());
  return new Promise((resolve) => awaitConnection({ browserId, ms, signal }, (ok) => resolve(ok && owned())));
}

/** What a wait for a browser is for. */
interface Wait {
  /** The browser. */
  browserId: string;
  /** How long to wait. */
  ms: number;
  /** Ends the wait early. */
  signal?: AbortSignal;
}

/** Tells `done` when the browser connects, or that it did not once the wait ends or is aborted. */
function awaitConnection(wait: Wait, done: (connected: boolean) => void) {
  const finish = (connected: boolean) => (stop(), done(connected));
  const heard = ({ id }) => id === wait.browserId && finish(true);
  const stop = watching(wait, heard, () => finish(false));
}

/** Listens for a connection, and for the wait ending or being aborted; answers how to stop listening to all three. */
function watching({ ms, signal }: Wait, heard: (browser) => void, gaveUp: () => void): () => void {
  const timer = setTimeout(gaveUp, ms);
  registry.on('browser:connected', heard);
  signal?.addEventListener('abort', gaveUp, { once: true });
  return () => {
    clearTimeout(timer);
    registry.off('browser:connected', heard);
    signal?.removeEventListener('abort', gaveUp);
  };
}
