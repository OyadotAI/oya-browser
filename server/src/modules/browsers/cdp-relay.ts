/**
 * CDP for Oya-client browsers, carried over the socket the browser dialled us on.
 *
 * A cloud sandbox or a desktop behind NAT has no endpoint we can dial, but it
 * already holds a socket to us. The gateway opens a relay on that socket and the
 * browser bridges it to its own CDP front door (cdp-front-door.js), so the
 * debug port never leaves the machine it runs on.
 *
 * A relay quacks like the `ws` socket gateway sessions expect: send, close,
 * readyState, and message/close events.
 */

import { EventEmitter } from 'events';
import { randomUUID } from 'crypto';
import { WebSocket } from 'ws';
import { RELAY_OPEN_TIMEOUT_MS } from './constants.ts';

/** Open relays by session id. */
const relays = new Map();

/** One CDP connection carried over a browser's control socket, duck-typing a `ws` WebSocket so CDP clients use it unchanged. */
class Relay extends EventEmitter {
  /** Relay session id, carried on every message. */
  declare readonly sid: string;
  /** The browser whose socket carries it. */
  declare readonly browserId: string;
  /** That browser's registry record; its socket is read at each send. */
  declare private readonly browser: any;
  /** CONNECTING, OPEN or CLOSED, as on a WebSocket. */
  declare readyState: number;
  /** Why it closed, when it did not close cleanly. */
  declare error: any;

  /** A relay for `browser`, not yet opened. */
  constructor(browser, browserId: string) {
    super();
    this.sid = randomUUID();
    this.browserId = browserId;
    this.browser = browser;
    this.readyState = WebSocket.CONNECTING;
  }

  /** Sends a relay message over the control socket; false when the socket is gone. */
  tell(msg: object) {
    try {
      this.browser.ws.send(JSON.stringify({ ...msg, sid: this.sid }));
      return true;
    } catch {
      return false;
    }
  }

  /** The browser confirmed the relay. */
  opened() {
    this.readyState = WebSocket.OPEN;
    this.emit('open');
  }

  /** Closes the relay locally, once, with an optional reason. */
  end(error?) {
    if (this.readyState === WebSocket.CLOSED) return;
    relays.delete(this.sid);
    this.readyState = WebSocket.CLOSED;
    this.error = error;
    this.emit('close');
  }

  /** Sends one CDP frame to the browser. */
  send(data) {
    if (!this.tell({ type: 'cdp', data: data.toString() })) this.end('Browser disconnected');
  }

  /** Asks the browser to close its end, then closes this one. */
  close() {
    if (this.readyState !== WebSocket.CLOSED) {
      this.tell({ type: 'cdp_close' });
      this.end();
    }
  }

  /** Same as close: there is nothing more abrupt to do. */
  terminate() {
    this.close();
  }
}

/** Resolves with the relay once the browser confirms; rejects if it refuses, the socket is gone, or time runs out. */
function whenOpen(relay: Relay, timeoutMs: number) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => relay.close(), timeoutMs);
    for (const event of ['open', 'close']) relay.once(event, () => clearTimeout(timer));
    relay.once('open', () => resolve(relay));
    relay.once('close', () => reject(new Error(relay.error || 'Browser did not open CDP')));
    if (!relay.tell({ type: 'cdp_open' })) relay.end('Browser disconnected');
  });
}

/** Asks the browser to open a CDP relay over its control socket; resolves with the relay once the browser confirms, rejects if it refuses or times out. */
export function openRelay(browser, browserId, timeoutMs = RELAY_OPEN_TIMEOUT_MS) {
  const relay = new Relay(browser, browserId);
  relays.set(relay.sid, relay);
  return whenOpen(relay, timeoutMs);
}

/** What each relay message does to its relay. */
const ON_MESSAGE: Record<string, (relay: Relay, msg) => void> = {
  cdp: (relay, msg) => relay.emit('message', Buffer.from(String(msg.data)), false),
  cdp_opened: (relay) => relay.opened(),
  cdp_closed: (relay, msg) => relay.end(msg.error),
};

/** A relay message arriving on browserId's control socket. */
export function onBrowserMessage(browserId, msg) {
  const relay = relays.get(msg.sid);
  if (!relay || relay.browserId !== browserId) return;
  if (Object.hasOwn(ON_MESSAGE, msg.type)) ON_MESSAGE[msg.type](relay, msg);
}

/** The browser's socket went away, so every relay on it did too. */
export function closeRelays(browserId) {
  for (const relay of [...relays.values()]) if (relay.browserId === browserId) relay.end('Browser disconnected');
}
