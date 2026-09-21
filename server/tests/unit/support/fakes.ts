/**
 * Test doubles shared by the unit tests: a socket that records what was sent,
 * and a registry entry for a connected browser.
 */
import { registry } from '../../../src/modules/browsers/registry.ts';

/** A WebSocket stand-in: records sends and closes, and can be told to fail. */
export class FakeSocket {
  /** Parsed messages written to the socket. */
  sent: any[] = [];
  /** The close code and reason, once closed. */
  closed: { code: number; reason: string } | null = null;
  /** ws's OPEN state value, compared against readyState. */
  readonly OPEN = 1;
  /** Open until closed. */
  readyState = 1;
  /** When set, send() throws it. */
  failWith: Error | null = null;

  /** Records the message, or throws `failWith`. */
  send(data: string) {
    if (this.failWith) throw this.failWith;
    this.sent.push(JSON.parse(data));
  }

  /** Records the close. */
  close(code: number, reason: string) {
    this.closed = { code, reason };
    this.readyState = 3;
  }

  /** Messages of one type. */
  ofType(type: string) {
    return this.sent.filter((m) => m.type === type);
  }
}

/** Registers a connected Oya browser for the duration of a test; returns its socket. */
export function connectBrowser(browserId: string, apiKey = 'key-a') {
  const ws = new FakeSocket();
  registry.add(browserId, { ws, apiKey, name: 'Test', clientType: 'oya', persona: { id: 'p-1' } });
  return ws;
}

/** Removes a browser registered by connectBrowser. */
export const disconnectBrowser = (browserId: string) => registry.remove(browserId);
