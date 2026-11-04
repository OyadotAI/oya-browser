/**
 * The one-shot listener a sign-in returns to: 127.0.0.1 on a port the OS picks,
 * answering /auth/callback once with a "close this tab" page and closing after
 * it, or when the sign-in has taken too long.
 */
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as C from './constants.ts';

/** A listener waiting for one sign-in to come back. */
export interface Callback {
  /** The address to return to, known once the listener is up. */
  redirectUri: string;
  /** The returned query, once the browser comes back. */
  done: Promise<Record<string, string>>;
  /** Stops listening and rejects a still-pending sign-in so cancellation settles callers. */
  close(): void;
}

/** Answers one request: the callback's page (and its query), or 404 for anything else. */
function answer(req: http.IncomingMessage, res: http.ServerResponse, settle: (q: Record<string, string>) => void) {
  const url = new URL(req.url || '/', `http://${C.LOOPBACK_HOST}`);
  if (url.pathname !== C.CALLBACK_PATH) return void res.writeHead(C.HTTP_NOT_FOUND).end();
  res.writeHead(C.HTTP_OK, { 'Content-Type': 'text/html; charset=utf-8' }).end(C.CALLBACK_PAGE);
  settle(Object.fromEntries(url.searchParams));
}

/** Starts listening; `ttlMs` bounds how long the sign-in may take. */
export function awaitCallback(ttlMs = C.LOGIN_TTL_MS): Promise<Callback> {
  const { promise: done, resolve: settle, reject: fail } = Promise.withResolvers<Record<string, string>>();
  const server = http.createServer((req, res) => answer(req, res, settle));
  const timer = setTimeout(() => fail(Error('Sign-in took too long. Try again.')), ttlMs);
  const cleanup = () => (clearTimeout(timer), server.close());
  const close = () => (fail(Error('Sign-in cancelled.')), cleanup());
  void done.then(cleanup, cleanup);
  return cleanupFailedListen(listening(server, done, close), close);
}

/** A bind failure releases the deadline as well as the server handle. */
function cleanupFailedListen(pending: Promise<Callback>, close: () => void): Promise<Callback> {
  return pending.catch((error) => {
    close();
    throw error;
  });
}

/** Resolves once the server listens, with the address it listens on. */
function listening(server: http.Server, done: Callback['done'], close: () => void): Promise<Callback> {
  return new Promise((resolve, reject) => {
    server.once('error', (e: NodeJS.ErrnoException) => reject(e.code === 'EADDRINUSE' ? Error(C.PORT_BUSY) : e));
    server.listen(C.LOOPBACK_PORT, C.LOOPBACK_HOST, () => {
      const { port } = server.address() as AddressInfo;
      resolve({ redirectUri: `http://${C.LOOPBACK_HOST}:${port}${C.CALLBACK_PATH}`, done, close });
    });
  });
}
