/**
 * The driver for an Oya browser: one that dialled this server and holds a
 * control socket open. Commands, live-view control and relayed CDP all travel
 * over that one socket.
 */
import type { Call } from '../connection/reporter.ts';
import { SocketTransport } from '../connection/transports/socket-transport.ts';
import { LIVE_VIEW_FPS } from '../connection/constants.ts';
import { openRelay } from '../cdp-relay.ts';
import { actionsFor } from '../../../drivers/vocabulary.ts';
import type { BrowserDriver, CdpEndpoint } from './port.ts';

/** What an Oya browser brings when it registers: its socket, and whether it offers CDP over it. */
type OyaLink = {
  /** The control socket the browser dialled us on. */
  ws: {
    /** Writes one message to the browser. */
    send(data: string): void;
  };
  /** Whether the browser relays CDP over that socket. */
  cdp?: boolean;
  /** The actions the browser said it does when it registered; absent from an older app. */
  actions?: readonly string[] | null;
};

/** Drives one Oya browser over its control socket. */
export class OyaDriver implements BrowserDriver {
  /** An Oya browser. */
  readonly kind = 'oya';
  /** It sends heartbeats, so a long silence means it is gone. */
  readonly heartbeat = true;
  /** The browser's socket, and whether it offers CDP. */
  declare private readonly link: OyaLink;
  /** The browser's id, which a relay carries on every message. */
  declare private readonly browserId: string;

  /** Wraps a browser that just registered on `link.ws`. */
  constructor(link: OyaLink, browserId: string) {
    this.link = link;
    this.browserId = browserId;
  }

  /** Sends the command over the socket; the browser's `cmd_result` settles it. */
  send(call: Call) {
    return new SocketTransport(this.link.ws).send(call);
  }

  /** A closed socket removes the browser from the registry, so one still here is alive. */
  isAlive() {
    return true;
  }

  /** Asks the browser to push frames; they arrive on the socket, so `onFrame` is not used. */
  async startScreencast() {
    this.tell({ type: 'stream_start', fps: LIVE_VIEW_FPS });
  }

  /** Asks the browser to stop pushing frames. */
  async stopScreencast() {
    this.tell({ type: 'stream_stop' });
  }

  /** Null: an Oya browser syncs its own cookie jar to its persona as it goes. */
  async cookies() {
    return null;
  }

  /** CDP over a relay on the control socket, when the browser said it offers one. */
  cdpEndpoint(): CdpEndpoint | null {
    if (!this.link.cdp) return null;
    return { open: () => openRelay(this.link, this.browserId) };
  }

  /** What the browser said it does, or the Oya list for an app too old to say. */
  actions() {
    return this.link.actions ?? actionsFor('oya');
  }

  /** Nothing to close: the browser opened the socket, and its closing is what removes it. */
  close() {}

  /** Writes one control message; a socket that just went away is not this caller's problem. */
  private tell(message: object) {
    try {
      this.link.ws.send(JSON.stringify(message));
    } catch {}
  }
}
