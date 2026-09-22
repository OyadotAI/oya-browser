/**
 * The browsers' control sockets: accepting a connection, and sending commands
 * to connected browsers. The work lives in connection/; this is the entry point
 * the rest of the server imports.
 */
import { BrowserConnection } from './connection/browser-connection.ts';
import './live-view.ts';
import { registry } from './registry.ts';

export { sendCommand, takeDialogNote } from './connection/commands.ts';

/** The actions a connected browser does, or null when it is not connected here. */
export const actionsOf = (browserId: string): readonly string[] | null =>
  registry.get(browserId)?.driver.actions() ?? null;

/** Takes over a new browser socket; it authenticates within 10s or is closed. */
export function handleConnection(ws, req?) {
  new BrowserConnection(ws, req);
}
