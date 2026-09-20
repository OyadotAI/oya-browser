/** Run the standard Next.js server behind the API's single public port. */
import { spawn } from 'node:child_process';
import { request } from 'node:http';
import { connect } from 'node:net';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { UI_DIR } from '../platform/paths.ts';
import { Status } from '../platform/http-status.ts';
import { DEFAULT_PORT, LOOPBACK, RAW_HEADER_STRIDE } from './constants.ts';

/** The modes OYA_UI_MODE may name. */
const UI_MODES = ['development', 'production'];

/**
 * Start Next.js on a loopback port (dev server or standalone build, per OYA_UI_MODE) and
 * return handlers that proxy HTTP requests and /_next/ websocket upgrades to it. Null when
 * OYA_UI_MODE is unset; the API exits if the frontend dies.
 */
export function startFrontend() {
  const mode = process.env.OYA_UI_MODE;
  if (!mode) return null; // API-only deployments and integration tests
  if (!UI_MODES.includes(mode)) throw new Error('OYA_UI_MODE must be development or production');
  const uiDir = UI_DIR + '/';
  const port = Number(process.env.OYA_UI_PORT || Number(process.env.PORT || DEFAULT_PORT) + 1);
  const entry = entryFor(mode, uiDir);
  if (!existsSync(entry)) throw new Error('Next.js is missing. Run npm run setup, then npm run build for production.');
  return handlersFor(port, supervise(spawnNext(mode, uiDir, entry, port)));
}

/** The Next.js script to run: its dev CLI, or the standalone server of a production build. */
function entryFor(mode, uiDir) {
  return mode === 'development'
    ? join(uiDir, 'node_modules/next/dist/bin/next')
    : join(uiDir, '.next/standalone/server.js');
}

/** Spawn Next.js on the loopback port, sharing this process's stdio. */
function spawnNext(mode, uiDir, entry, port) {
  const args = mode === 'development' ? ['dev', uiDir, '--hostname', LOOPBACK, '--port', String(port)] : [];
  return spawn(process.execPath, [entry, ...args], {
    env: { ...process.env, PORT: String(port), HOSTNAME: LOOPBACK, NODE_ENV: mode },
    stdio: 'inherit',
  });
}

/** Stop the child when this process exits, and take this process down if the child fails or exits on its own. */
function supervise(child) {
  let stopping = false;
  const stop = () => {
    stopping = true;
    child.kill('SIGTERM');
  };
  process.once('exit', stop);
  watch(child, () => stopping);
  return stop;
}

/** Shut down when the child fails to start, or exits while `stopping()` is false. */
function watch(child, stopping: () => boolean) {
  child.once('error', onChildError);
  child.once('exit', () => stopping() || onChildExit());
}

/** Next.js could not start: shut down. */
function onChildError(err) {
  console.error('[next]', err.message);
  process.kill(process.pid, 'SIGTERM');
}

/** Next.js exited without being asked to: shut down with a failure code. */
function onChildExit() {
  console.error('[next] Frontend exited; shutting down');
  process.exitCode = 1;
  process.kill(process.pid, 'SIGTERM');
}

/** What the API calls to stop the frontend and hand it requests and upgrades. */
function handlersFor(port, stop) {
  return {
    stop,
    handle: (req, res) => proxyRequest(port, req, res),
    upgrade: (req, socket, head) => proxyUpgrade(port, req, socket, head),
  };
}

/** Proxy one HTTP request to Next.js, streaming both ways. */
function proxyRequest(port, req, res) {
  const upstream = request(
    { hostname: LOOPBACK, port, path: req.originalUrl, method: req.method, headers: req.headers },
    (response) => relayResponse(response, res),
  );
  upstream.on('error', () => answerStarting(res));
  req.on('aborted', () => upstream.destroy());
  res.on('close', () => upstream.destroy());
  req.pipe(upstream); // Preserve streaming, RSC headers and Server Action bodies.
}

/** Copy Next.js's response to the client. */
function relayResponse(response, res) {
  res.writeHead(response.statusCode, response.headers);
  response.pipe(res);
  response.on('error', () => res.destroy());
}

/** Next.js is not answering yet: 503 with a retry hint, or drop a response already under way. */
function answerStarting(res) {
  if (res.headersSent) return res.destroy();
  res.writeHead(Status.UNAVAILABLE, { 'Content-Type': 'text/plain', 'Retry-After': '2' });
  res.end('Oya is starting. Please retry in a moment.\n');
}

/** Proxy a /_next/ websocket upgrade to Next.js; false for any other path. */
function proxyUpgrade(port, req, socket, head) {
  if (!req.url.startsWith('/_next/')) return false;
  const upstream = connect(port, LOOPBACK, () => replayUpgrade(upstream, req, socket, head));
  upstream.on('error', () => socket.destroy());
  socket.on('error', () => upstream.destroy());
  socket.on('close', () => upstream.destroy());
  return true;
}

/** Re-send the upgrade request line, headers and any early bytes, then join the two sockets. */
function replayUpgrade(upstream, req, socket, head) {
  upstream.write(`${req.method} ${req.url} HTTP/${req.httpVersion}\r\n`);
  for (let i = 0; i < req.rawHeaders.length; i += RAW_HEADER_STRIDE)
    upstream.write(`${req.rawHeaders[i]}: ${req.rawHeaders[i + 1]}\r\n`);
  upstream.write('\r\n');
  if (head.length) upstream.write(head);
  socket.pipe(upstream).pipe(socket);
}
