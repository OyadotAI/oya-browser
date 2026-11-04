/** A one-use, state-bound loopback callback returns a pairing code without exposing a workspace key. */
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import { timingSafeEqual } from 'node:crypto';
import * as C from './constants.ts';

/** An owned listener has one result and explicit cancellation. */
export interface DesktopCallback {
  /** Only this literal loopback URL is handed to the login page. */ url: string;
  /** A validated one-use pairing code, never a workspace credential. */ done: Promise<string>;
  /** Closes outstanding connections and settles pending work. */ close(): void;
}
/** Compare fixed-size secrets without giving a caller a prefix oracle. */
function sameState(given: string, expected: string): boolean {
  const a = Buffer.from(given),
    b = Buffer.from(expected);
  return a.length === b.length && timingSafeEqual(a, b);
}
/** Strict callback validation rejects forged hosts, duplicate parameters and unexpected fields. */
function codeOf(req: http.IncomingMessage, host: string, state: string): string | null {
  if (!admitsRequest(req, host)) return null;
  const url = URL.parse(req.url!, `http://${host}`);
  if (!url || !callbackFields(url)) return null;
  const code = url.searchParams.get('code') || '';
  return code && sameState(url.searchParams.get('state') || '', state) ? code : null;
}
/** Exactly two fields prevent duplicate-state ambiguity and destination overrides. */
function callbackFields(url: URL): boolean {
  return url.pathname === C.DESKTOP_CALLBACK_PATH && [...url.searchParams.keys()].sort().join(',') === 'code,state';
}
/** Only bounded origin-form callback requests reach the nonthrowing URL parser. */
function admitsRequest(req: http.IncomingMessage, host: string): boolean {
  return (
    req.method === 'GET' &&
    req.headers.host === host &&
    (req.url?.length || 0) <= C.DESKTOP_CALLBACK_MAX_CHARS &&
    !!req.url?.startsWith(`${C.DESKTOP_CALLBACK_PATH}?`)
  );
}
/** Static responses never include a credential or untrusted value. */
const HEADERS = {
  'Content-Type': 'text/html; charset=utf-8',
  'Cache-Control': 'no-store',
  'Referrer-Policy': 'no-referrer',
  'Content-Security-Policy': "default-src 'none'; frame-ancestors 'none'",
};
/** The page reports receipt rather than falsely claiming socket readiness. */
const RECEIVED =
  '<!doctype html><title>Return to Oya</title><h1>Return to Oya Browser</h1><p>Your sign-in was received. You can close this tab.</p>';
/** A rejected request can be retried without disclosing why a state failed. */
const INVALID = 'This sign-in return is invalid or expired. Return to Oya and try again.';
/** State for one listener is private and consumed at most once. */
interface PendingCallback {
  /** Bound listener. */ server: http.Server;
  /** Native secret. */ state: string;
  /** Owner lifetime. */ signal: AbortSignal;
  /** One pairing-code result. */ result: ReturnType<typeof Promise.withResolvers<string>>;
  /** Prevent concurrent valid replays. */ used: boolean;
}
/** Start a bounded listener; invalid requests never consume the genuine attempt. */
export function desktopCallback(state: string, signal: AbortSignal): Promise<DesktopCallback> {
  const context = { server: http.createServer(), state, signal, result: Promise.withResolvers<string>(), used: false };
  context.server.on('request', (req, res) => answerDesktopReturn(res, consume(context, req)));
  return openCallback(context);
}
/** Only a matching callback consumes the one-use result. */
function consume(c: PendingCallback, req: http.IncomingMessage): boolean {
  const host = `${C.DESKTOP_CALLBACK_HOST}:${(c.server.address() as AddressInfo)?.port}`;
  const code = !c.used && !c.signal.aborted && codeOf(req, host, c.state);
  if (!code) return false;
  c.used = true;
  c.result.resolve(code);
  return true;
}
/** Responses contain no user-controlled text. */
function answerDesktopReturn(res: http.ServerResponse, accepted: boolean): void {
  res.writeHead(accepted ? C.HttpStatus.OK : C.HttpStatus.BAD_REQUEST, HEADERS).end(accepted ? RECEIVED : INVALID);
}
/** Bind failures and cancellation close the listener before it can return to a caller. */
async function openCallback(c: PendingCallback): Promise<DesktopCallback> {
  const close = () => cancelCallback(c);
  installLifetime(c, close);
  await bind(c).catch((error) => {
    close();
    throw error;
  });
  const port = (c.server.address() as AddressInfo).port;
  return { url: `http://${C.DESKTOP_CALLBACK_HOST}:${port}${C.DESKTOP_CALLBACK_PATH}`, done: c.result.promise, close };
}
/** Cancellation settles callers and destroys incomplete callback requests. */
function cancelCallback(c: PendingCallback): void {
  c.result.reject(Error('Sign-in didn’t finish. Try again.'));
  c.server.close();
  c.server.closeAllConnections();
}
/** Completion removes abort subscriptions, including rejected bind attempts. */
function installLifetime(c: PendingCallback, close: () => void): void {
  const cleanup = () => {
    c.signal.removeEventListener('abort', close);
    c.server.close();
    c.server.closeIdleConnections();
  };
  void c.result.promise.then(cleanup, cleanup);
  c.signal.addEventListener('abort', close, { once: true });
}
/** Aborting during binding cannot publish a listener after its owner has gone. */
async function bind(c: PendingCallback): Promise<void> {
  c.signal.throwIfAborted();
  await new Promise<void>((resolve, reject) => {
    c.server.once('error', reject);
    c.server.listen(C.DESKTOP_CALLBACK_PORT, C.DESKTOP_CALLBACK_HOST, resolve);
  });
  c.signal.throwIfAborted();
}
