/**
 * CDP front door for automation harnesses (OYA_REMOTE_DEBUGGING_PORT).
 *
 * Electron's own debug endpoint is not something an agent can use as-is: it
 * lists this browser's UI as a page an agent will happily drive, and
 * Target.createTarget answers "Not supported". This proxies Chromium's endpoint
 * (one port up, loopback only), hides the UI, and opens tabs through createTab
 * so every page an agent gets has taken the protected path, fingerprint,
 * proxy and persona partition included.
 *
 * This file is the facade and the HTTP side; door.ts knows the targets and
 * bridge.ts carries each WebSocket.
 */
import http, { type IncomingHttpHeaders, type IncomingMessage, type Server, type ServerResponse } from 'node:http';
import { isIP } from 'node:net';
import type { Duplex } from 'node:stream';
import { WebSocketServer } from 'ws';
import { isWebAddress, NOT_A_WEB_ADDRESS } from '../tabs/navigation.ts';
import { FrontDoor, isUi } from './door.ts';
import { Bridge } from './bridge.ts';
import { FRONT_DOOR_MAX_PAYLOAD, Status } from './constants.ts';
import type { FrontDoorOptions } from './types.ts';

export { isUi };
export type { FrontDoorOptions };

/** The only endpoints a validation run may call over HTTP. */
const RUN_ENDPOINTS = ['/json/version', '/json/protocol', '/json', '/json/list'];

/** Anything with request headers: an HTTP request, or a WebSocket upgrade. */
interface HeldRequest {
  /** The request's headers. */
  headers: IncomingHttpHeaders;
}

/** What one HTTP request's routes share. */
interface Exchange {
  /** The request. */
  req: IncomingMessage;
  /** Writes a JSON answer; a string body is sent as it is. */
  send: (status: number, body: unknown) => void;
  /** Points Chromium's URLs in `text` back at this door. */
  rewrite: (text: string) => string;
}

/** One HTTP route. */
type Route = (door: FrontDoor, exchange: Exchange, path: string) => Promise<void>;

/** A WebSocket upgrade, as the server hands it over. */
interface Upgrade {
  /** The upgrade request. */
  req: IncomingMessage;
  /** The raw socket under it. */
  socket: Duplex;
  /** The first packet of the upgraded stream. */
  head: Buffer;
}

/**
 * Chromium refuses a debug request whose Host is neither an IP literal nor
 * localhost, and refuses a WebSocket upgrade carrying an Origin. Both checks
 * live on the endpoint this proxy re-issues requests to, `fetch` below sends
 * Host: 127.0.0.1, and the `ws` client sends no Origin, so they are lost
 * unless the front door makes them itself.
 *
 * Without the Host check, a page the user visits points a name it controls at
 * 127.0.0.1, reaches this port *same-origin*, reads /json/list and opens a CDP
 * socket onto tabs the persona is signed into.
 */
export const localHost = (req: HeldRequest): boolean => {
  const raw = req.headers.host || '';
  const host = raw.startsWith('[') ? raw.slice(1, raw.indexOf(']')) : raw.split(':')[0];
  return host === 'localhost' || isIP(host) !== 0;
};

/** Writes a JSON answer; a string body is sent as it is. */
function answerJson(res: ServerResponse, status: number, body: unknown): void {
  res.writeHead(status, { 'Content-Type': 'application/json' });
  res.end(typeof body === 'string' ? body : JSON.stringify(body));
}

/** The helpers one HTTP request's routes share: `send`, and `rewrite` to point Chromium's URLs back at this door. */
function httpExchange(door: FrontDoor, req: IncomingMessage, res: ServerResponse): Exchange {
  const reqHost = req.headers.host || `127.0.0.1:${door.options.port}`;
  const upstreamHost = `localhost:${door.options.upstream}`;
  return {
    req,
    send: (status, body) => answerJson(res, status, body),
    rewrite: (text) => text.split(door.up).join(reqHost).split(upstreamHost).join(reqHost),
  };
}

/** /json/new: opens a tab the protected way. */
async function openFromHttp(door: FrontDoor, { req, send, rewrite }: Exchange): Promise<void> {
  // PUT-only, as Chromium made it: a page cannot send one, so an <img> or
  // a form cannot open a tab in the persona on the victim's behalf.
  // eslint-disable-next-line no-magic-numbers -- regressions.js reads this line as written
  if (req.method !== 'PUT') return send(405, { error: '/json/new requires PUT' });
  const url = decodeURIComponent((req.url ?? '').split('?')[1] || '') || 'about:blank';
  if (!isWebAddress(url)) return send(Status.BAD_REQUEST, { error: NOT_A_WEB_ADDRESS });
  const targetId = await door.admitted(() => door.openTab(url));
  const target = (await door.refreshHidden()).find((t) => t.id === targetId);
  return send(Status.OK, rewrite(JSON.stringify(target || { id: targetId })));
}

/** /json and /json/list: Chromium's list without the hidden targets. */
async function listFromHttp(door: FrontDoor, { send, rewrite }: Exchange): Promise<void> {
  const list = await door.refreshHidden();
  return send(Status.OK, rewrite(JSON.stringify(list.filter((t) => !door.hidden.has(t.id ?? '')))));
}

/** Anything else goes to Chromium as it is; only /json/version and /json/protocol skip admission. */
async function proxyFromHttp(door: FrontDoor, { req, send, rewrite }: Exchange, path: string): Promise<void> {
  const free = path === '/json/version' || path === '/json/protocol';
  const forward = async (): Promise<void> => {
    const upRes = await fetch(`http://${door.up}${req.url}`, { method: req.method });
    send(upRes.status, rewrite(await upRes.text()));
  };
  return free ? forward() : door.admitted(forward);
}

/** Path → handler; any other path is proxied. */
const HTTP_ROUTES: Record<string, Route> = {
  '/json/new': openFromHttp,
  '/json': listFromHttp,
  '/json/list': listFromHttp,
};

/** Routes one request that passed the Host and run checks. */
async function routeHttp(door: FrontDoor, exchange: Exchange): Promise<void> {
  const path = (exchange.req.url ?? '').split('?')[0].replace(/\/$/, '');
  if (door.options.runToken && !RUN_ENDPOINTS.includes(path)) {
    return exchange.send(Status.FORBIDDEN, { error: 'Endpoint unavailable for validation' });
  }
  const route = Object.hasOwn(HTTP_ROUTES, path) ? HTTP_ROUTES[path] : proxyFromHttp;
  return route(door, exchange, path);
}

/** One HTTP request: the run token and Host checks, then the route; a failure answers 502. */
async function handleHttp(door: FrontDoor, req: IncomingMessage, res: ServerResponse): Promise<void> {
  const exchange = httpExchange(door, req, res);
  const { send } = exchange;
  if (door.options.runToken && req.headers['x-oya-run'] !== door.options.runToken) {
    return send(Status.FORBIDDEN, { error: 'Run authentication required' });
  }
  // eslint-disable-next-line no-magic-numbers -- regressions.js reads this line as written
  if (!localHost(req)) return send(403, { error: 'Host header must be an IP address or localhost' });
  await routeHttp(door, exchange).catch((e: Error) => send(Status.BAD_GATEWAY, { error: e.message }));
}

/** A WebSocket upgrade: local, Origin-less, authenticated for a run, and onto a visible target; any other is destroyed. */
async function handleUpgrade(door: FrontDoor, wss: WebSocketServer, { req, socket, head }: Upgrade): Promise<unknown> {
  if (!localHost(req) || req.headers.origin) return socket.destroy();
  if (door.options.runToken && req.headers['x-oya-run'] !== door.options.runToken) return socket.destroy();
  const m = (req.url ?? '').match(/^\/devtools\/(browser|page)\/([^/?]+)/);
  if (!(await door.tryRefresh())) return socket.destroy();
  if (!m || door.hidden.has(m[2])) return socket.destroy();
  const relay = !!door.options.relayToken && req.headers['x-oya-relay'] === door.options.relayToken;
  const url = `ws://${door.up}${req.url}`;
  wss.handleUpgrade(req, socket, head, (client) => new Bridge(door, client, url, m[1] === 'browser', relay));
}

/** The front door closed: drop every bridged harness. */
function closeBridges(wss: WebSocketServer): void {
  for (const client of wss.clients) client.terminate();
  wss.close();
}

/**
 * Starts the front door on `host:port` in front of Chromium on `upstream`.
 * `tabs`, `createTab` and `closeTab` are the app's; `beginCommand` admits a
 * command, `clientChanged` counts local clients; `relayToken` marks the
 * server's relay; `runToken` and `allowedTarget` scope a validation run.
 */
export function start(options: FrontDoorOptions): Server {
  const door = new FrontDoor(options);
  const server = http.createServer((req, res) => void handleHttp(door, req, res));
  const wss = new WebSocketServer({ noServer: true, perMessageDeflate: false, maxPayload: FRONT_DOOR_MAX_PAYLOAD });
  server.on('upgrade', (req, socket, head) => void handleUpgrade(door, wss, { req, socket, head }));
  server.on('close', () => closeBridges(wss));
  const listening = () => console.log(`[cdp] front door on ${options.host}:${options.port} → ${door.up}`);
  server.listen(options.port, options.host, listening);
  return server;
}
