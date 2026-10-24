/**
 * Letting go of a browser's registration: what a socket that closed, a socket a
 * reconnect replaced and a browser the server dropped must all give back. One
 * routine for every path, so a resource added to one cannot be forgotten on another.
 */
import { registry } from '../registry.ts';
import { destroyMcpServer } from '../../../mcp/server.ts';
import { metrics } from '../../../platform/metrics.ts';
import * as usage from '../../../platform/usage.ts';
import { container } from '../../../app/container.ts';
import { closeRelays } from '../cdp-relay.ts';
import { failPending } from './commands.ts';

/** The parts of a registry record that hold something to give back. */
export interface Registered {
  /** The key whose usage clock is running for the browser. */
  apiKey: string;
  /** The persona whose concurrency slot the browser holds. */
  persona?: any;
}

/**
 * Fails the browser's pending commands, ends its relays, unlists it, stops its
 * usage clock and frees its persona slot. Called by whoever takes the browser
 * out of the registry, at that moment: the socket's own close comes later (a
 * real ws reports it once the closing handshake ends, or after its close timeout
 * when the peer went quiet), when the browser is no longer its to release.
 * Left to that close, a browser that was replaced, dropped by an operator or
 * revoked key, or stopped by the control plane and did not come back stayed
 * billed and kept its persona slot until the server restarted; an accepted
 * reconnect lost the old socket's unbooked time; and relayed CDP clients hung.
 */
export function releaseBrowser(browserId: string, browser: Registered, outcome: 'disconnected' | 'reconnected') {
  failPending(browserId, outcome, outcome === 'reconnected' ? 'Browser reconnected' : 'Browser disconnected');
  closeRelays(browserId);
  registry.remove(browserId);
  destroyMcpServer(browserId);
  usage.browserDisconnected(browser.apiKey, browserId);
  container.personas.release(browser.persona, browserId);
  metrics.browsersConnected.set({}, registry.browsers.size);
}
