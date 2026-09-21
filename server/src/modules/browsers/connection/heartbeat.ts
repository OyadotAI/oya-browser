/**
 * Liveness in both directions: the server pings, and closes a socket whose
 * pongs have stopped.
 */
import { CloseCode, MISSED_PINGS, PING_INTERVAL_MS } from './constants.ts';

/** Pings one socket and closes it once pongs stop. */
export class Heartbeat {
  /** The socket being watched. */
  declare private readonly ws: any;
  /** Name for the log. */
  declare private readonly label: () => string;
  /** When the browser was last heard from. */
  declare private lastHeard: number;
  /** The ping loop, once started. */
  declare private timer: ReturnType<typeof setInterval> | null;

  /** Watches `ws`; `label` names the browser in the log. */
  constructor(ws, label: () => string) {
    this.ws = ws;
    this.label = label;
    this.lastHeard = Date.now();
    this.timer = null;
  }

  /** The browser said something: it is alive. */
  heard() {
    this.lastHeard = Date.now();
  }

  /** Pings on an interval until stopped. */
  start() {
    this.timer = setInterval(() => this.beat(), PING_INTERVAL_MS);
  }

  /** Stops pinging. */
  stop() {
    clearInterval(this.timer);
  }

  /** One ping, or a close when the browser has gone quiet too long. */
  private beat() {
    if (Date.now() - this.lastHeard > PING_INTERVAL_MS * MISSED_PINGS) return this.expire();
    try {
      this.ws.send(JSON.stringify({ type: 'ping' }));
    } catch {
      this.stop();
    }
  }

  /** Pongs stopped: close the socket. */
  private expire() {
    console.log(`[ws] Browser ${this.label()} missed pongs, closing`);
    this.stop();
    this.ws.close(CloseCode.PONG_TIMEOUT, 'Pong timeout');
  }
}
