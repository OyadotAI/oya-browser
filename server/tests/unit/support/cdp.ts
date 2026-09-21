/**
 * A CDP browser without Chrome: a connection that records every command and
 * answers from the test, and a CDPDriver attached to it.
 */
import { CDPDriver } from '../../../src/drivers/cdp/driver.ts';

/** One command the fake connection received. */
export type CdpCall = { method: string; params: any; sessionId?: string };

/** The session and target the fake driver is attached to. */
export const SESSION = 'session-1';
/** The attached target. */
export const TARGET = 'target-1';
/** The isolated world's execution context. */
export const WORLD = 7;

/** A CDPConnection stand-in: `replies` answers by method; everything else answers {}. */
export class FakeCDPConnection {
  /** Commands sent, in order. */
  calls: CdpCall[] = [];
  /** Set by close(). */
  closed = false;
  /** Event listeners by method. */
  listeners = new Map<string, Set<(params: any, sessionId?: string) => void>>();
  /** Answers by CDP method: a value, or a function of the params. An Error is thrown. */
  replies: Record<string, any> = {
    'Page.getFrameTree': { frameTree: { frame: { id: 'frame-1' } } },
    'Page.createIsolatedWorld': { executionContextId: WORLD },
    'Runtime.evaluate': (params: any) => ({ result: { value: this.evaluate(params.expression, params) } }),
  };
  /** What a Runtime.evaluate returns, by expression; set by the test. */
  evaluate: (expression: string, params: any) => any = () => undefined;
  /** The WebSocket stand-in session code listens on. */
  ws = { on: () => {} };

  /** Records the command and answers it. */
  async send(method: string, params: any = {}, sessionId?: string) {
    this.calls.push({ method, params, sessionId });
    const reply = this.replies[method];
    const value = typeof reply === 'function' ? await reply(params, sessionId) : reply;
    if (value instanceof Error) throw value;
    return value ?? {};
  }

  /** Subscribes to an event; returns the unsubscribe. */
  on(method: string, fn: (params: any, sessionId?: string) => void) {
    if (!this.listeners.has(method)) this.listeners.set(method, new Set());
    this.listeners.get(method)!.add(fn);
    return () => this.listeners.get(method)?.delete(fn);
  }

  /** Resolves at once, as for a page that has already loaded. */
  async once() {
    return null;
  }

  /** Fires an event at its listeners. */
  emit(method: string, params: any = {}, sessionId: string = SESSION) {
    for (const fn of this.listeners.get(method) || []) fn(params, sessionId);
  }

  /** Marks the connection closed. */
  close() {
    this.closed = true;
  }

  /** The calls of one method. */
  sent(method: string) {
    return this.calls.filter((c) => c.method === method);
  }

  /** The method names sent, in order, leaving out the analyzer's world setup and evaluations. */
  methods() {
    const setup = ['Page.getFrameTree', 'Page.createIsolatedWorld', 'Runtime.evaluate'];
    return this.calls.map((c) => c.method).filter((m) => !setup.includes(m));
  }
}

/** A CDPDriver attached to a fake connection, watching dialogs as attach() leaves it. */
export function fakeDriver(options: any = {}) {
  const conn = new FakeCDPConnection();
  const driver = new CDPDriver(options);
  Object.assign(driver, { conn, sessionId: SESSION, targetId: TARGET, tagAttr: 'data-test' });
  driver.watchDialogs();
  return { driver, conn };
}
