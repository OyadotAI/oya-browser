/**
 * The browsers' control sockets: accepting a connection, and sending commands
 * to connected browsers. The work lives in connection/; this is the entry point
 * the rest of the server imports.
 */
import { BrowserConnection } from './connection/browser-connection.ts';
import './live-view.ts';

export { sendCommand, takeDialogNote } from './connection/commands.ts';

/** Takes over a new browser socket; it authenticates within 10s or is closed. */
export function handleConnection(ws, req?) {
  new BrowserConnection(ws, req);
}
