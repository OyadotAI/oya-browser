/**
 * Oya Browser server, HTTP + WebSocket + MCP API.
 * API routes are served under /api; Next.js handles the frontend at /.
 */
import { migrateLegacy } from './modules/control/migrate.ts';
import { createEgressServer } from './modules/control/egress.ts';
import { control } from './modules/control/service.ts';
import { forwardHttp } from './modules/control/cluster.ts';
import { startWorkers, stopWorkers, workerHealth } from './modules/control/worker.ts';

import 'dotenv/config';
import { startFrontend } from './app/frontend.ts';

// Prevent crashes from unhandled errors once we are serving. Before that, a
// failure is a failed boot: every hard check below runs at top level, and a
// top-level-await rejection arrives here rather than as a crash. Swallowing it
// would exit 0 with nothing listening, which reads to an orchestrator (or to
// `oya install`) as a successful start that then vanished.
let booted = false;
const onBootFailure = (label, err) => {
  console.error(`[oya] ${label} during boot:`, err?.message || err);
  console.error('[oya] refusing to start.');
  process.exit(1);
};
process.on('uncaughtException', (err) => {
  if (!booted) return onBootFailure('Uncaught exception', err);
  console.error('[oya] Uncaught exception:', err.message);
});
process.on('unhandledRejection', (reason: any) => {
  if (!booted) return onBootFailure('Unhandled rejection', reason);
  console.error('[oya] Unhandled rejection:', reason?.message || reason);
});

import express from 'express';
import { createServer } from 'http';
import { WebSocketServer } from 'ws';
import { join } from 'path';
import cors from 'cors';
import { router as apiRouter } from './app/api.ts';
import { slackActionsRouter } from './modules/slack/service.ts';
import { drain as drainAudit } from './platform/audit.ts';
import {
  handleJsonVersion,
  handleJsonList,
  handleUpgrade as handleGatewayUpgrade,
  sessions as gatewaySessions,
} from './modules/gateway/service.ts';
import * as usage from './platform/usage.ts';
import { container } from './app/container.ts';
import * as keyConfig from './modules/config/service.ts';
import { drain as drainLogins } from './modules/personas/cookies.ts';
import { handleConnection } from './modules/browsers/socket.ts';
import { handleMcpRequest, handlePoolMcpRequest } from './mcp/server.ts';
import { validateApiKey, authReady } from './modules/auth/service.ts';
import { registry } from './modules/browsers/registry.ts';
import { pool } from './modules/gateway/routing.ts';
import { PUBLIC_DIR, DOWNLOADS_DIR } from './platform/paths.ts';
import { Status } from './platform/http-status.ts';
import { DECIMAL, DEFAULT_PORT, INVALID_KEY_CLOSE_CODE } from './app/constants.ts';

// Not yet layered: reads the persona service from the composition root.
const { personas } = container;

const PORT = parseInt(process.env.PORT || String(DEFAULT_PORT), DECIMAL);

// Browsers treat an invalid key as fatal. Never accept a reconnect while the
// persisted key cache or the profile bound to it is still being restored.
const [authLoaded] = await Promise.all([authReady, usage.restore(), personas.restore(), keyConfig.restore()]);
if (!authLoaded) throw new Error('API keys could not be loaded; refusing to accept browser connections');

keyConfig.restoreRouting(pool);

await control().store.get('meta', 'draining'); // fail fast when control storage is unreachable
await migrateLegacy();
const app = express();
app.get('/livez', (req, res) => res.json({ status: 'ok' }));
app.get('/readyz', async (req, res) => {
  try {
    const draining = (await control().store.get('meta', 'draining'))?.value;
    const ready = !draining && !registry.draining && !workerHealth.lastError;
    res.status(ready ? Status.OK : Status.UNAVAILABLE).json({ ready });
  } catch {
    res.status(Status.UNAVAILABLE).json({ ready: false });
  }
});
startWorkers();
const egressServer = process.env.OYA_EGRESS_PORT ? createEgressServer() : null;
egressServer?.listen(Number(process.env.OYA_EGRESS_PORT), process.env.OYA_EGRESS_HOST || '127.0.0.1');

app.use(cors());

// ── Legacy domain redirect: old hosts → canonical host ──
// The old hosts still resolve and terminate TLS at the ingress; anything
// human-facing gets pushed to the canonical domain. /ws, /mcp, /api and
// /downloads pass through untouched so already-installed browsers and MCP
// clients configured against the old host keep working.
const LEGACY_HOSTS: Record<string, string> = {
  'browser.getoya.ai': 'oyabrowser.com',
  'browser.oya.ai': 'oyabrowser.com',
  'www.oyabrowser.com': 'oyabrowser.com',
  'dev-browser.getoya.ai': 'dev.oyabrowser.com',
  'dev-browser.oya.ai': 'dev.oyabrowser.com',
};
const REDIRECT_EXEMPT = ['/ws', '/mcp', '/api', '/downloads'];

app.use((req, res, next) => {
  const host = (req.headers.host || '').split(':')[0].toLowerCase();
  if (!Object.hasOwn(LEGACY_HOSTS, host)) return next();
  if (REDIRECT_EXEMPT.some((p) => req.path === p || req.path.startsWith(p + '/'))) return next();
  // 308 rather than 301, preserves method and body for non-GET requests
  res.redirect(Status.PERMANENT_REDIRECT, `https://${LEGACY_HOSTS[host]}${req.originalUrl}`);
});

const publicDir = PUBLIC_DIR;

// ── Discovery & docs (root level) ──
app.use('/.well-known', express.static(join(publicDir, '.well-known')));
// One file, several names. Crawlers look for different ones and a second copy
// would only drift from this.
for (const path of ['/llms.txt', '/llms-full.txt', '/docs.txt']) {
  app.get(path, (req, res) => res.type('text/plain').sendFile(join(publicDir, 'llms.txt')));
}
app.get('/openapi.json', (req, res) => res.type('application/json').sendFile(join(publicDir, 'openapi.json')));

// ── REST API under /api ──
// CDP discovery. Playwright, Puppeteer, Stagehand and browser-use fetch these
// before connecting, which is what lets them treat the gateway as a browser.
app.get('/json/version', handleJsonVersion);
app.get('/json/list', handleJsonList);

// 15mb, not the 100kb default: a task file in `data` rides inline as base64, which
// inflates a 10MB upload to ~13.4MB. Every /api route is behind authMiddleware.
// The SDK caps one file at 10MB, so what lands here is a run carrying several at once;
// express's own answer is an HTML stack trace, which reads as a server fault rather than
// a request that asked for too much.
// Slack posts its interactivity payloads form-encoded and signs the raw bytes, so
// this one path is parsed before, and differently from, every other /api route.
app.use(
  '/api/slack/actions',
  express.urlencoded({
    extended: false,
    verify: (req: express.Request, _res, buf) => {
      req.rawBody = buf;
    },
  }),
  slackActionsRouter,
);

app.use(
  '/api',
  express.json({ limit: '15mb' }),
  (err, req, res, next) => {
    if (err?.type !== 'entity.too.large') return next(err);
    res
      .status(Status.PAYLOAD_TOO_LARGE)
      .json({ error: 'This request is over 15MB. Task files ride inline, so send fewer or smaller ones in one run.' });
  },
  (req, res, next) => {
    req.body ??= {};
    next();
  },
  apiRouter,
); // Express 5 leaves body undefined without a JSON payload; handlers destructure it
// Prometheus convention is /metrics at the root; the same handler also serves
// /api/metrics for callers that prefix everything.
app.get(['/health', '/metrics'], apiRouter);

// ── Downloads (binary files) ──
// The folder itself has no page: static would add a slash and Next.js strip it again, forever.
// Send it to the landing page's per-platform download buttons instead.
app.get(['/downloads', '/downloads/'], (req, res) => res.redirect(Status.FOUND, '/#download'));
app.use('/downloads', express.static(DOWNLOADS_DIR));

// ── MCP endpoints (root level, clients connect directly) ──
// A per-browser MCP request is served by the replica holding that browser.
app.use('/mcp', express.json(), forwardHttp);
app.post('/mcp/pool', handlePoolMcpRequest);
app.get('/mcp/pool', handlePoolMcpRequest);
app.delete('/mcp/pool', handlePoolMcpRequest);
app.post('/mcp/:browserId', handleMcpRequest);
app.get('/mcp/:browserId', handleMcpRequest);
app.delete('/mcp/:browserId', handleMcpRequest);

// ── Next.js runtime (optional for API-only hosts) ──
const frontend = startFrontend();
if (frontend) app.use((req, res) => frontend.handle(req, res));
else
  app.use((req, res) =>
    res
      .status(Status.NOT_FOUND)
      .json({ error: 'Not found. Run npm run dev from the repository root to start the console.' }),
  );

const server = createServer(app);

// Disable HTTP server timeout so long-running commands aren't killed
server.timeout = 0;
server.requestTimeout = 0;

// ── WebSocket at /ws ──
const wss = new WebSocketServer({
  noServer: true,
  perMessageDeflate: false,
});

// One upgrade router: /ws is the Oya client protocol, /connect is raw CDP.
server.on('upgrade', (req, socket, head) => {
  const { pathname } = new URL(req.url, `http://${req.headers.host}`);
  if (pathname === '/ws') {
    return wss.handleUpgrade(req, socket, head, (ws) => wss.emit('connection', ws, req));
  }
  if (pathname === '/connect') return upgradeGateway(req, socket, head);
  if (frontend?.upgrade(req, socket, head)) return;
  console.warn(`[ws] ✗ upgrade to unknown path ${pathname}, use /ws (Oya client) or /connect (CDP)`);
  socket.destroy();
});

/** Hand a /connect upgrade to the CDP gateway; a failure is logged and the socket dropped. */
function upgradeGateway(req, socket, head) {
  return handleGatewayUpgrade(req, socket, head).catch((e) => {
    console.error('[gateway] upgrade failed:', e.message);
    try {
      socket.destroy();
    } catch {}
  });
}

wss.on('connection', (ws, req) => {
  const url = new URL(req.url, `http://${req.headers.host}`);
  const apiKey = url.searchParams.get('key');

  if (apiKey && !validateApiKey(apiKey)) {
    ws.close(INVALID_KEY_CLOSE_CODE, 'Invalid API key');
    return;
  }

  handleConnection(ws, req);
});

// Log browser events
registry.on('browser:connected', ({ id, name }) => {
  console.log(`[oya] Browser connected: ${name} (${id})`);
});

registry.on('browser:disconnected', ({ id, name }) => {
  console.log(`[oya] Browser disconnected: ${name} (${id})`);
});

for (const signal of ['SIGTERM', 'SIGINT']) {
  process.once(signal, async () => {
    registry.draining = true;
    egressServer?.close();
    await stopWorkers();
    frontend?.stop();
    // End gateway sessions cleanly so profiles are captured and recordings
    // get their manifest, rather than being cut off mid-write.
    await Promise.allSettled([...gatewaySessions.values()].map((s) => s.destroy('server shutting down')));
    await Promise.allSettled([drainAudit(), drainLogins(), usage.drain(), personas.drain(), keyConfig.drain()]);
    process.exit(process.exitCode || 0);
  });
}

server.listen(PORT, () => {
  booted = true;
  console.log(`[oya] Oya Browser server listening on port ${PORT}`);
  console.log(`[oya] UI:           http://localhost:${PORT}/`);
  console.log(`[oya] API:          http://localhost:${PORT}/api/`);
  console.log(`[oya] WebSocket:    ws://localhost:${PORT}/ws`);
  console.log(`[oya] MCP endpoint: http://localhost:${PORT}/mcp/:browserId`);
  console.log(`[oya] MCP pool:     http://localhost:${PORT}/mcp/pool`);
  console.log(`[oya] CDP gateway:  ws://localhost:${PORT}/connect  (discovery: /json/version)`);
});
