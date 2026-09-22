/**
 * What the server needs from a browser, whatever drives it. An Oya browser
 * dialled us and is reached over its control socket; a vendor's browser we
 * dialled ourselves over CDP. Callers hold a `BrowserDriver` and never ask
 * which kind it is: every difference between the two lives behind a member
 * here, so there is one place to look when a third kind arrives.
 */
import type { Call } from '../connection/reporter.ts';
import type { CommandResult } from '../connection/transports/transport.ts';

/**
 * An open CDP socket, shaped like a `ws` WebSocket: send, close, readyState,
 * and message and close events. A dialled WebSocket is one; so is a relay
 * carried over an Oya browser's control socket.
 */
export type CdpSocket = any;

/** Where Playwright or the gateway can speak raw CDP to this browser. */
export interface CdpEndpoint {
  /** The address a vendor's browser is dialled at; absent when CDP travels over a relay. */
  readonly url?: string;
  /** Opens a CDP socket to the browser; rejects when it cannot be reached. */
  open(): Promise<CdpSocket>;
}

/** One connected browser, as the rest of the server drives it. */
export interface BrowserDriver {
  /** Which kind this is, for messages and metrics only. Branching on it defeats the port. */
  readonly kind: 'oya' | 'cdp';
  /** Whether the browser reports in by itself, so that silence means trouble. */
  readonly heartbeat: boolean;
  /** Runs one command and resolves with the browser's answer; rejects on a timeout or a lost connection. */
  send(call: Call): Promise<CommandResult>;
  /** Whether the connection this driver holds is still open. */
  isAlive(): boolean;
  /** Starts live-view frames. `onFrame` is called only by a driver that pulls frames itself. */
  startScreencast(onFrame: (dataUrl: string) => void): Promise<void>;
  /** Stops live-view frames. */
  stopScreencast(): Promise<void>;
  /** The browser's cookies, to save into its persona before it stops; null when the browser syncs its own jar. */
  cookies(): Promise<object[] | null>;
  /** Where raw CDP can reach this browser, or null when it cannot. */
  cdpEndpoint(): CdpEndpoint | null;
  /** The actions this browser does, sorted, one spelling each: what the browser detail lists. */
  actions(): readonly string[];
  /** Lets go of whatever this server opened to reach the browser. */
  close(): void;
}
