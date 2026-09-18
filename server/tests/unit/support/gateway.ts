/**
 * Test doubles for the CDP gateway: the control plane's slot and lifecycle
 * calls replaced with recorders, WebSocket stand-ins for a gateway client and
 * a browser, and a loopback CDP server that answers like a browser.
 */
import { mock } from 'node:test';
import { EventEmitter } from 'node:events';
import { WebSocketServer } from 'ws';
import type { AddressInfo } from 'node:net';
import { control } from '../../../src/modules/control/service.ts';

/**
 * Replaces the control-plane calls the gateway makes with recorders: provider
 * holds are numbered, commands get a release function, and lifecycle calls
 * resolve. Returns the calls; `mock.restoreAll()` puts the real ones back.
 */
export function stubControl() {
  const calls: { method: string; args: any[] }[] = [];
  let holds = 0;
  const record =
    (method: string, answer: (...args: any[]) => any = () => undefined) =>
    async (...args: any[]) => {
      calls.push({ method, args });
      return answer(...args);
    };
  const c: any = control();
  mock.method(
    c,
    'holdProvider',
    record('holdProvider', () => `hold-${++holds}`),
  );
  mock.method(c, 'releaseProvider', record('releaseProvider'));
  mock.method(
    c,
    'beginCommand',
    record('beginCommand', () => mock.fn(async () => {})),
  );
  mock.method(c, 'update', record('update'));
  mock.method(c, 'emit', record('emit'));
  mock.method(c, 'complete', record('complete'));
  const of = (method: string) => calls.filter((call) => call.method === method);
  return { calls, of };
}

/** A ws socket stand-in: an EventEmitter that records sends and closes. */
export class FakeWs extends EventEmitter {
  /** Messages sent, as given. */
  sent: any[] = [];
  /** The close code and reason, once closed. */
  closed: { code?: number; reason?: string } | null = null;
  /** ws readyState; OPEN (1) until closed. */
  readyState = 1;
  /** When set, send() throws it. */
  failWith: Error | null = null;

  /** Records a message. */
  send(data: any) {
    if (this.failWith) throw this.failWith;
    this.sent.push(data);
  }

  /** Records the close. */
  close(code?: number, reason?: string) {
    this.closed = { code, reason };
    this.readyState = 3;
  }

  /** Sent messages parsed as JSON. */
  json() {
    return this.sent.map((m) => JSON.parse(m.toString()));
  }
}

/** How the fake browser answers one CDP command: a result, or throw for an error. */
export type CdpAnswer = (method: string, params: any, sessionId?: string) => any;

/**
 * A browser's CDP endpoint on loopback. Answers each command through `answer`
 * (an empty result by default) and records it; `emit` sends an event to every
 * connected client.
 */
export async function fakeCdp(answer: CdpAnswer = () => ({})) {
  const commands: { method: string; params: any; sessionId?: string }[] = [];
  const wss = new WebSocketServer({ port: 0, host: '127.0.0.1' });
  await new Promise((resolve) => wss.once('listening', resolve));
  wss.on('connection', (ws) =>
    ws.on('message', (raw) => {
      const { id, method, params, sessionId } = JSON.parse(raw.toString());
      commands.push({ method, params, sessionId });
      try {
        ws.send(JSON.stringify({ id, sessionId, result: answer(method, params, sessionId) ?? {} }));
      } catch (e) {
        ws.send(JSON.stringify({ id, sessionId, error: { message: (e as Error).message } }));
      }
    }),
  );
  const url = `ws://127.0.0.1:${(wss.address() as AddressInfo).port}/devtools/browser/fake`;
  const emit = (method: string, params: any, sessionId?: string) =>
    wss.clients.forEach((c) => c.send(JSON.stringify({ method, params, sessionId })));
  const close = () =>
    new Promise<void>((resolve) => {
      wss.clients.forEach((c) => c.terminate());
      wss.close(() => resolve());
    });
  return { url, commands, emit, close, clients: () => wss.clients.size };
}

/** A browser with one page, attached as session "s-1"; `extra` answers anything else. */
export const pageBrowser =
  (extra: CdpAnswer = () => ({})): CdpAnswer =>
  (method, params, sessionId) => {
    if (method === 'Target.getTargets') return { targetInfos: [{ type: 'page', targetId: 't-1' }] };
    if (method === 'Target.attachToTarget') return { sessionId: 's-1' };
    return extra(method, params, sessionId);
  };
