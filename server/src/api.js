/**
 * REST API routes.
 */

import { Router } from 'express';
import { randomBytes, timingSafeEqual } from 'crypto';
import {
  authMiddleware, userAuthMiddleware,
  registerApiKey, listApiKeys, deleteApiKey, keyDigest,
  provisionKeys,
  signup, login, getProfile, updateProfile,
} from './auth.js';
import { registry } from './connection-registry.js';
import { sendCommand } from './ws-handler.js';
import { runChat } from './chat-service.js';
import { runtimeConfig } from './runtime-config.js';
import { nextBrowser, poolStats } from './pool.js';
import { getAll as getAllCookies, clear as clearCookies, getStorage, mergeStorage, mergeDump, drain as drainLogins } from './cookie-store.js';
import { isConfigured as sandboxConfigured, missingSettings, createSandbox, removeSandbox, listSandboxBrowsers } from './sandbox.js';
import { metrics, render as renderMetrics, snapshot as metricsSnapshot } from './metrics.js';
import { audit, history as auditHistory, fingerprint } from './audit.js';
import * as usage from './usage.js';
import { enforce, consume, checkQuota, checkHourly, status as limitStatus, LIMITS, QUOTAS } from './limits.js';
import { acquire as acquireBrowser, available as availableProviders } from './providers.js';
import { CDPDriver } from './drivers/cdp.js';
import { assertSafeTarget } from './net-guard.js';
import { v4 as uuidv4 } from 'uuid';
import { listSessions, killSession, sessions as gatewaySessions } from './gateway.js';
import { pool, STRATEGIES, validateProviderConfig } from './routing.js';
import * as profiles from './profiles.js';
import * as recorder from './recorder.js';
import * as personas from './personas.js';
import * as proxies from './proxies.js';
import * as captcha from './captcha.js';
import * as mfa from './mfa.js';
import * as playbooks from './playbook.js';
import * as flow from './flow-recorder.js';
import * as runs from './runs.js';
import * as keyConfig from './key-config.js';
import { slackRouter } from './slack.js';
import { PREF_OPTIONS } from './fingerprint.js';
import * as pairing from './pairing.js';

import { control, live } from './control/service.js';
import { forwardHttp } from './control/cluster.js';
import { createManaged, managedConfigured, removeManaged } from './control/managed.js';
import { admission } from './control/admission.js';
import { projectAccountRouter } from './control/membership.js';
import { controlRouter } from './control/routes.js';

// caseSensitive: Express matches routes case-insensitively by default, but the
// role guards in authMiddleware test req.path — which keeps the client's
// casing. `GET /api/Pool/Cookies` would otherwise route to the handler while
// slipping past the guard that names `/pool/cookies`.
export const router = Router({ caseSensitive: true });
for (const method of ['get', 'post', 'put', 'patch', 'delete']) {
  const register = router[method].bind(router);
  router[method] = (path, ...handlers) => register(path, ...handlers.map(handler => handler.constructor.name === 'AsyncFunction' ? (req, res, next) => Promise.resolve(handler(req, res, next)).catch(next) : handler));
}
router.use(forwardHttp);
router.use('/control', controlRouter);
router.use('/slack', slackRouter);
router.use('/auth/projects', projectAccountRouter);

async function browserCdpUrl(req, id) {
  const host = req.headers.host || `localhost:${process.env.PORT || 3100}`;
  const scheme = req.secure || String(req.headers['x-forwarded-proto'] || '').includes('https') ? 'wss' : 'ws';
  return `${scheme}://${host}/connect?ticket=${encodeURIComponent(await control().ticket(getKey(req), id, req.authToken || getKey(req)))}&browser=${encodeURIComponent(id)}`;
}

/**
 * HTTP metrics. Labelled by the route pattern, never the concrete path — at
 * this fleet size /browsers/:id/command must not become one series per browser.
 */
router.use((req, res, next) => {
  const started = Date.now();
  res.on('finish', () => {
    const route = (req.route?.path || req.path)
      .replace(/\/[0-9a-f]{8}-[0-9a-f-]{27,}/gi, '/:id')
      .replace(/\/[A-Za-z0-9_-]{24,}/g, '/:token');
    metrics.httpRequests.inc({ route, status: `${Math.floor(res.statusCode / 100)}xx` });
    metrics.httpDuration.observe({ route }, Date.now() - started);
  });
  next();
});

// One pair of listeners for the whole process. Registering per browser would
// leak a listener each time and trip EventEmitter's max at 11 CDP browsers.
registry.on('stream:start', ({ id }) => {
  const browser = registry.get(id);
  if (!browser?.driver?.startScreencast) return;   // Oya clients push frames themselves
  browser.driver.startScreencast((dataUrl) => {
    registry.pushFrame(id, dataUrl);
    metrics.frames.inc({ client: 'cdp' });
  }).catch(() => {});
});
registry.on('stream:stop', ({ id }) => {
  registry.get(id)?.driver?.stopScreencast?.().catch(() => {});
});

/** Everything is scoped to the calling key. There is no tier above it. */
const ownerScope = (req) => fingerprint(getKey(req));

/**
 * Process-level controls (Prometheus scrape, drain) are host operations, not
 * tenant data, so they are gated on an explicitly-named operator token rather
 * than on any API key. An API key never confers power over the host, and no
 * env var silently turns a key into a superuser.
 */
function operatorOnly(req, res, next) {
  const token = process.env.OYA_OPERATOR_TOKEN || process.env.OYA_METRICS_TOKEN;
  // Header only: a token in the query string lands in access logs, proxy logs
  // and browser history. Prometheus sends an Authorization header natively.
  const supplied = getKey(req);
  if (token && supplied) {
    const a = Buffer.from(supplied);
    const b = Buffer.from(token);
    if (a.length === b.length && timingSafeEqual(a, b)) return next();
  }
  return res.status(403).json({
    error: 'Host controls need OYA_OPERATOR_TOKEN in the Authorization header',
  });
}

/** Extract API key from Authorization header */
function getKey(req) {
  return req.headers.authorization?.slice(7) || '';
}

/** A browser belongs to the key that connected it. Nothing else can reach it. */
function canAccess(req, browserId) {
  return registry.belongsTo(browserId, getKey(req));
}

// Health check (no auth required)
router.get('/health', (req, res) => {
  res.json({
    status: 'ok',
    browsers: registry.list().length,
    uptime: process.uptime(),
  });
});

// ─── User Auth ────────────────────────────────────────────────────────────────

// ─── Session cookie ──────────────────────────────────────────────────────────

/**
 * The refresh token is the long-lived half of a session, so it travels in an
 * httpOnly cookie that page JavaScript cannot read. In localStorage it handed
 * any XSS in the console a way to hold the session open forever.
 *
 * SameSite=Lax means the cookie only comes back to its own origin, so a console
 * served from a different origin than the API — NEXT_PUBLIC_API_URL pointed
 * elsewhere during development — would never send it. In that case the token is
 * returned in the body as before and the caller stores it; `refresh_in_cookie`
 * tells the client which of the two happened.
 */
const REFRESH_COOKIE = 'oya_rt';
const REFRESH_COOKIE_PATH = '/api/auth';
/**
 * A readable companion to the httpOnly cookie above, holding nothing but the
 * fact that a session exists. Page JavaScript cannot see `oya_rt`, so without
 * this the console has to POST /auth/refresh on every anonymous page load just
 * to find out there is nothing to refresh.
 */
const SESSION_HINT = 'oya_session';

function sameOrigin(req) {
  const origin = req.headers.origin;
  if (!origin) return true;                 // same-origin fetch, or a non-browser client
  try { return new URL(origin).host === req.headers.host; } catch { return false; }
}

function readRefreshCookie(req) {
  const raw = (req.headers.cookie || '').split(';')
    .map((c) => c.trim())
    .find((c) => c.startsWith(`${REFRESH_COOKIE}=`));
  if (!raw) return '';
  try { return decodeURIComponent(raw.slice(REFRESH_COOKIE.length + 1)); } catch { return ''; }
}

function issueSession(req, res, result) {
  const { refresh_token: refresh, ...rest } = result;
  if (!refresh || !sameOrigin(req)) return res.json(result);
  const secure = req.secure || String(req.headers['x-forwarded-proto'] || '').includes('https');
  const maxAge = 30 * 24 * 3600 * 1000;
  res.cookie(REFRESH_COOKIE, refresh, { httpOnly: true, sameSite: 'lax', secure, path: REFRESH_COOKIE_PATH, maxAge });
  res.cookie(SESSION_HINT, '1', { httpOnly: false, sameSite: 'lax', secure, path: '/', maxAge });
  res.json({ ...rest, refresh_in_cookie: true });
}

function clearSessionCookies(res) {
  res.clearCookie(REFRESH_COOKIE, { path: REFRESH_COOKIE_PATH });
  res.clearCookie(SESSION_HINT, { path: '/' });
}

router.post('/auth/signup', async (req, res) => {
  const { email, password, display_name } = req.body;
  if (!email || !password) {
    return res.status(400).json({ error: 'email and password required' });
  }
  if (password.length < 8) {
    return res.status(400).json({ error: 'Password must be at least 8 characters' });
  }
  try {
    const result = await signup(email, password, display_name);
    res.json(result);
  } catch (err) {
    res.status(400).json({ error: err.message });
  }
});

router.post('/auth/login', async (req, res) => {
  const { email, password } = req.body;
  if (!email || !password) {
    return res.status(400).json({ error: 'email and password required' });
  }
  try {
    const result = await login(email, password);
    issueSession(req, res, result);
  } catch (err) {
    res.status(401).json({ error: err.message });
  }
});

router.post('/auth/refresh', async (req, res) => {
  const presented = req.body?.refresh_token || readRefreshCookie(req);
  if (!presented) {
    return res.status(400).json({ error: 'refresh_token required' });
  }
  try {
    const { refreshSession } = await import('./auth.js');
    const result = await refreshSession(presented);
    issueSession(req, res, result);
  } catch (err) {
    clearSessionCookies(res);
    res.status(401).json({ error: err.message });
  }
});

/** Signing out has to reach the cookie, which the page cannot clear itself. */
router.post('/auth/logout', (req, res) => {
  clearSessionCookies(res);
  res.json({ ok: true });
});

router.get('/auth/me', userAuthMiddleware, async (req, res) => {
  try {
    const profile = await getProfile(req.user.id);
    res.json(profile);
  } catch (err) {
    res.status(500).json({ error: err.message });
  }
});

/** Rename yourself. Email is the login and role is authority; neither moves here. */
router.patch('/auth/me', userAuthMiddleware, async (req, res) => {
  try {
    const profile = await updateProfile(req.user.id, { display_name: req.body?.display_name });
    audit({ action: 'account.update', actorKey: null, targetType: 'user', targetId: req.user.id, req });
    res.json(profile);
  } catch (err) {
    res.status(err.status || 500).json({ error: err.message });
  }
});

// ─── API Key Management (authenticated users) ────────────────────────────────

router.get('/auth/keys', userAuthMiddleware, async (req, res) => {
  try {
    const keys = await listApiKeys(req.user.id);
    res.json(keys);
  } catch (err) {
    res.status(500).json({ error: err.message });
  }
});

// The only time the key itself is returned. Only its digest is stored, so
// there is no second chance to read it and nothing to hand back later.
router.post('/auth/keys', userAuthMiddleware, async (req, res) => {
  const { label } = req.body;
  try {
    const key = randomBytes(24).toString('base64url');
    await registerApiKey(key, req.user.id, label);
    res.json({ key, id: keyDigest(key), prefix: key.slice(0, 8), project: control().projectIdFor(key), label: label || 'Default' });
  } catch (err) {
    res.status(500).json({ error: err.message });
  }
});

router.post('/auth/keys/import', userAuthMiddleware, async (req, res) => {
  const { key, label } = req.body;
  // An imported key becomes an administrator credential over a project holding
  // cookie jars, MFA seeds and proxy credentials, so it has to be as hard to
  // guess as one this server mints (randomBytes(24) — 32 base64url chars).
  // Without this, importing "a" was a valid, publicly guessable admin key.
  if (!key || typeof key !== 'string' || !/^[A-Za-z0-9_-]{32,128}$/.test(key)) {
    return res.status(400).json({ error: 'key must be 32-128 characters of A-Z a-z 0-9 _ -' });
  }
  try {
    await registerApiKey(key, req.user.id, label || 'Imported');
    res.json({ ok: true, id: keyDigest(key), prefix: key.slice(0, 8), project: control().projectIdFor(key), label: label || 'Imported' });
  } catch (err) {
    res.status(err.status || 500).json({ error: err.message });
  }
});

// By digest, which is what GET /auth/keys returns as `id`. The server cannot
// look a key up by its plaintext any more, and a URL is the last place to put
// one anyway.
router.delete('/auth/keys/:id', userAuthMiddleware, async (req, res) => {
  try {
    await deleteApiKey(req.params.id, req.user.id);
    res.json({ ok: true });
  } catch (err) {
    res.status(500).json({ error: err.message });
  }
});

// Batch-provision API keys. Minting credentials is a host capability — a
// tenant key must not be able to mint more identities. Accounts still create
// their own keys through /auth/keys.
router.post('/fleet/provision', operatorOnly, async (req, res) => {
  const key = getKey(req);
  const count = Math.min(Math.max(parseInt(req.query.count || req.body?.count) || 1, 1), 10000);
  const keys = await provisionKeys(count);
  audit({ action: 'key.provision', actorKey: key, targetType: 'key', meta: { count: keys.length }, req });
  res.json({ ok: true, count: keys.length, keys });
});

// ─── Observability ───────────────────────────────────────────────────────────

/**
 * Prometheus scrape target. Accepts an admin key or a dedicated
 * OYA_METRICS_TOKEN, so a scraper does not need admin credentials.
 */
router.get('/metrics', operatorOnly, (req, res) => {
  metrics.browsersConnected.set({}, registry.browsers.size);
  res.type('text/plain; version=0.0.4').send(renderMetrics());
});

/**
 * Everything this key owns, in one call: its browsers, sessions, providers,
 * usage and allowance. This is the dashboard's fleet view. There is no admin
 * variant, because the key is the whole identity.
 */
router.get('/fleet', authMiddleware, async (req, res) => {
  const key = getKey(req);
  const owner = fingerprint(key);
  const mine = await listSandboxBrowsers(key, registry.list(key));
  const byClient = {};
  const byProvider = {};
  const byHealth = { ok: 0, stale: 0, errors: 0, dead: 0 };
  const byPersona = {};
  let commands = 0, errors = 0, pending = 0;
  for (const row of mine) {
    const b = row;
    byClient[b.clientType || 'oya'] = (byClient[b.clientType || 'oya'] || 0) + 1;
    if (b.provider) byProvider[b.provider] = (byProvider[b.provider] || 0) + 1;
    byHealth[row.health] = (byHealth[row.health] || 0) + 1;
    const p = row.personaName || row.persona || '—';
    byPersona[p] = (byPersona[p] || 0) + 1;
    commands += b.commands; errors += b.errors; pending += b.pending;
  }
  const sessions = listSessions(key);
  res.json({
    at: new Date().toISOString(),
    uptimeSeconds: Math.round(process.uptime()),
    browsers: { total: mine.length, byClient, byProvider, byHealth, byPersona, commands, errors, pending },
    sessions: {
      total: sessions.length,
      attached: sessions.filter((s) => s.connected).length,
      recording: sessions.filter((s) => s.recording).length,
    },
    routing: pool.stats(owner),
    usage: usage.current(key),
    ...limitStatus(key),
  });
});

/** The caller's own usage and remaining allowance — no admin key needed. */
router.get('/usage', authMiddleware, async (req, res) => {
  const key = getKey(req);
  const mine = [...registry.browsers.values()].filter((b) => b.apiKey === key).length;
  res.json({
    current: usage.current(key),
    browsers: { connected: mine, quota: QUOTAS.browsers },
    ...limitStatus(key),
    history: (await usage.history(key, { hours: Number(req.query.hours) || 24 })).rows,
  });
});

/** Audit trail. Admin only: it spans every tenant by construction. */
/** This key's own audit trail. */
router.get('/audit', authMiddleware, async (req, res) => {
  const result = await auditHistory({
    limit: Math.min(Number(req.query.limit) || 100, 1000),
    action: req.query.action,
    actor: fingerprint(getKey(req)),   // never another key's history
    since: req.query.since,
  });
  res.json(result);
});

// ─── Control ─────────────────────────────────────────────────────────────────

/**
 * Stop one browser, whatever it is. This is what the dashboard's Stop does.
 *
 *   oya-cloud   destroy the Oya Cloud sandbox (or it redials and keeps billing),
 *               then drop the socket and the registry entry
 *   cdp         registry.remove(), which closes the driver and releases the
 *               vendor session
 *   desktop     close the socket
 *
 * Returns what actually happened, so a Stop that could not reach Oya Cloud is
 * visible rather than reported as done.
 */
export async function stopBrowser(req, browserId, { sandbox, force = false } = {}) {
  const key = getKey(req);
  const browser = registry.get(browserId);
  if (!browser) {
    const gateway = gatewaySessions.get(browserId);
    if (gateway?.apiKey === key) { await killSession(browserId); return { id: browserId, ok: true }; }
    const durable = await control().findSession(key, browserId);
    if (durable && ['stopped', 'failed'].includes(durable.state)) return { id: browserId, ok: true, status: durable.state };
    // Queued work stops at once; force reconciles a resource the system has no way to delete.
    if (durable && !['stopped', 'failed'].includes(durable.state)) {
      const { state } = await control().cancel(key, browserId, { force });
      return { id: browserId, ok: true, status: state };
    }
    if (sandboxConfigured()) {
      try {
        const removed = await removeSandbox(browserId, key);
        if (removed) {
          audit({ action: 'browser.stop', actorKey: key, targetType: 'browser', targetId: browserId, meta: { provider: 'oya-cloud', sandboxRemoved: true }, req });
          return { id: browserId, ok: true, sandboxRemoved: true, provider: 'oya-cloud' };
        }
      } catch (err) { return { id: browserId, ok: false, error: err.message }; }
    }
    return { id: browserId, ok: false, error: 'Browser not connected' };
  }
  if (!canAccess(req, browserId)) return { id: browserId, ok: false, error: 'Browser not connected' };
  if (browser.driver && browser.persona) {
    try {
      const { cookies } = await browser.driver.conn.send('Network.getAllCookies', {}, browser.driver.sessionId);
      mergeDump(browser.persona.id, cookies);
      await drainLogins();
    } catch (err) {
      if (!force) return { id: browserId, ok: false, error: `Could not save profile before stopping: ${err.message}` };
      audit({ action: 'profile.capture.failed', actorKey: key, targetId: browserId, outcome: 'error' });
    }
  }

  // Whether a sandbox exists is decided by asking Oya Cloud, not by what the
  // browser said about itself: an older image sends no provider, and a stop
  // that trusts the claim leaves a sandbox running and billing. removeSandbox
  // looks the sandbox up by this browser's name and this key's owner label,
  // so for a desktop browser it simply finds nothing.
  let sandboxRemoved = null;
  const managedSession = (await control().findSession(key, browserId))?.runtime;
  if (managedSession) {
    await control().cancel(key, browserId);
    return { id: browserId, ok: true, status: 'cleanup_pending' };
  }
  const mightHaveSandbox = browser.clientType === 'oya' && (sandboxConfigured() || sandbox === true);
  if (mightHaveSandbox) {
    try {
      sandboxRemoved = await removeSandbox(browserId, key);
    } catch (err) {
      sandboxRemoved = false;
      console.warn(`[stop] sandbox for ${browserId} not removed: ${err.message}`);
    }
  }
  const durable = await control().findSession(key, browserId);
  if (durable) await control().update(key, browserId, { state: 'cleanup_pending' });
  if (browser.release) {
    try { await browser.release(); }
    catch (err) { return { id: browserId, ok: false, status: 502, error: err.message }; }
    browser.release = null;
  }
  try { browser.ws?.close(4008, 'Stopped by operator'); } catch {}
  registry.remove(browserId);
  if (durable && sandboxRemoved !== false) await control().update(key, browserId, { state: 'stopped' });
  usage.browserDisconnected(key, browserId);
  audit({ action: 'browser.stop', actorKey: key, targetType: 'browser', targetId: browserId,
    meta: { clientType: browser.clientType, provider: browser.provider, sandboxRemoved }, req });
  metrics.browsersConnected.set({}, registry.browsers.size);
  return {
    id: browserId, ok: true, sandboxRemoved,
    // If Oya Cloud had a sandbox for it, it was a cloud browser whatever it claimed.
    provider: sandboxRemoved ? 'oya-cloud' : browser.provider,
  };
}

router.post('/browsers/:browserId/stop', authMiddleware, async (req, res) => {
  const result = await stopBrowser(req, req.params.browserId, { sandbox: req.body?.sandbox, force: req.body?.force === true });
  res.status(result.ok ? 200 : (result.status || 404)).json(result);
});

/** Bulk stop: `{ids: [...]}` or `{all: true}`. Each id reports separately. */
router.post('/browsers/stop', authMiddleware, async (req, res) => {
  const key = getKey(req);
  const ids = req.body?.all === true
    ? [...new Set([...(await listSandboxBrowsers(key, registry.list(key))).map(b => b.id), ...[...gatewaySessions.values()].filter(s => s.apiKey === key && !s.attachedTo).map(s => s.id), ...(await control().sessions(key, live)).map(s => s.id)])]
    : (Array.isArray(req.body?.ids) ? req.body.ids.map(String).slice(0, 5000) : []);
  if (!ids.length) return req.body?.all === true
    ? res.json({ ok: true, stopped: 0, results: [] })
    : res.status(400).json({ error: 'Pass ids: [...] or all: true' });
  // Sandboxes are deleted over the network; a few at a time keeps a 1k-browser
  // "stop all" from opening a thousand connections to Oya Cloud at once.
  const results = [];
  for (let i = 0; i < ids.length; i += 8) {
    results.push(...await Promise.all(ids.slice(i, i + 8).map((id) => stopBrowser(req, id))));
  }
  res.json({ ok: true, stopped: results.filter((r) => r.ok).length, results });
});

/** One browser with its recent activity — what the detail panel polls. */
router.get('/browsers/:browserId', authMiddleware, async (req, res) => {
  const { browserId } = req.params;
  if (!registry.isConnected(browserId) || !canAccess(req, browserId)) {
    const cloud = (await listSandboxBrowsers(getKey(req))).find(row => row.id === browserId);
    if (cloud) return res.json({ ...cloud, activity: [] });
    return res.status(404).json({ error: `Browser ${browserId} not connected` });
  }
  const detail = registry.describe(browserId);
  if ((detail.clientType === 'cdp' || registry.get(browserId)?.cdp) && req.principal?.role !== 'viewer') detail.cdpUrl = await browserCdpUrl(req, browserId);
  res.json(detail);
});

/** Force a browser off the fleet — a stuck client, a runaway, an abusive key. */
router.post('/browsers/:browserId/disconnect', authMiddleware, (req, res) => {
  const { browserId } = req.params;
  const browser = registry.get(browserId);
  if (!browser || !canAccess(req, browserId)) return res.status(404).json({ error: 'Browser not connected' });
  try { browser.ws?.close(4008, 'Disconnected by operator'); } catch {}
  registry.remove(browserId);
  audit({ action: 'browser.disconnect', actorKey: getKey(req), targetType: 'browser', targetId: browserId,
    meta: { clientType: browser.clientType, reason: req.body?.reason || null }, req });
  metrics.browsersConnected.set({}, registry.browsers.size);
  res.json({ ok: true, disconnected: browserId });
});

/** Drop every browser belonging to one key, without waiting for key deletion. */
/** Drop every browser on the calling key. */
router.post('/browsers/disconnect-all', authMiddleware, (req, res) => {
  const target = getKey(req);
  const ids = [...registry.browsers.entries()].filter(([, b]) => b.apiKey === target).map(([id]) => id);
  for (const id of ids) {
    try { registry.get(id)?.ws?.close(4008, 'Key disconnected by operator'); } catch {}
    registry.remove(id);
  }
  audit({ action: 'key.disconnect', actorKey: getKey(req), targetType: 'key', targetId: fingerprint(target),
    meta: { browsers: ids.length }, req });
  metrics.browsersConnected.set({}, registry.browsers.size);
  res.json({ ok: true, disconnected: ids.length });
});

/**
 * Drain: stop accepting new browsers so this instance can be restarted without
 * dropping in-flight work. Read by the WebSocket handler.
 */
router.post('/operator/drain', operatorOnly, async (req, res) => {
  const draining = req.body?.draining !== false;
  await control().drain(draining);
  registry.draining = draining;
  audit({ action: draining ? 'fleet.drain' : 'fleet.undrain', actorKey: getKey(req), targetType: 'fleet', req });
  res.json({ ok: true, draining, connected: registry.browsers.size });
});

// ─── Browser providers (CDP) ─────────────────────────────────────────────────

router.get('/providers', authMiddleware, (req, res) => {
  res.json({ providers: availableProviders(keyConfig.envFor(getKey(req))), oyaCloud: sandboxConfigured() });
});

/**
 * Attach a CDP browser: one we dial out to, rather than one that dials in.
 * Anchor, Browserbase, Steel, or any Chrome with --remote-debugging-port.
 */
router.post('/browsers/connect', authMiddleware, enforce('connect'), admission('cdp'), async (req, res) => {
  const key = getKey(req);
  const mine = [...registry.browsers.values()].filter((b) => b.apiKey === key).length;
  const quota = checkQuota('browsers', key, mine);
  if (!quota.allowed) {
    audit({ action: 'browser.connect', actorKey: key, targetType: 'browser', outcome: 'denied',
      meta: { reason: 'quota', quota: quota.quota }, req });
    return res.status(429).json({ error: `Browser quota reached (${quota.quota})`, ...quota });
  }

  const provider = String(req.body?.provider || 'cdp');
  let session;
  try {
    if (provider === 'cdp' && req.body?.wsUrl) await assertSafeTarget(req.body.wsUrl, { label: 'wsUrl' });
    session = await acquireBrowser({ provider, wsUrl: req.body?.wsUrl, env: keyConfig.envFor(key), onCreated: cleanup => control().update(key, req.controlSession.id, { cleanup }) });
  } catch (err) {
    audit({ action: 'browser.connect', actorKey: key, targetType: 'browser', outcome: 'error',
      meta: { provider, error: err.message }, req });
    return res.status(err.status || 502).json({ error: err.message });
  }

  const browserId = req.controlSession?.id || uuidv4();
  try {
    if (session.cleanup) await control().update(key, browserId, { cleanup: session.cleanup });
    await control().assertProvisioning(key, browserId);
    const driver = await new CDPDriver({
      wsUrl: session.wsUrl,
      provider: session.provider,
      onClose: () => { if (registry.get(browserId)) registry.remove(browserId); },
    }).connect();

    registry.add(browserId, {
      apiKey: key,
      name: (req.body?.name || `${session.provider} browser`).slice(0, 100),
      driver,
      clientType: 'cdp',
      provider: session.provider,
      release: session.release,
    });

    usage.browserConnected(key, browserId);
    metrics.wsConnections.inc({ outcome: 'ok' });
    metrics.browsersConnected.set({}, registry.browsers.size);
    audit({ action: 'browser.connect', actorKey: key, targetType: 'browser', targetId: browserId,
      meta: { provider: session.provider, sessionId: session.sessionId }, req });

    res.status(201).json({ id: browserId, provider: session.provider, clientType: 'cdp' });
  } catch (err) {
    await session.release().catch((err) => console.error('[providers] cleanup:', err.message));
    audit({ action: 'browser.connect', actorKey: key, targetType: 'browser', outcome: 'error',
      meta: { provider, error: err.message }, req });
    res.status(502).json({ error: `Could not attach to the CDP browser: ${err.message}` });
  }
});

/** Detach a CDP browser and release the vendor session. */
router.delete('/browsers/:browserId/connection', authMiddleware, async (req, res) => {
  const { browserId } = req.params;
  const browser = registry.get(browserId);
  if (!browser || !canAccess(req, browserId)) return res.status(404).json({ error: 'Browser not found' });
  const result = await stopBrowser(req, browserId);
  if (!result.ok) return res.status(result.status || 404).json(result);
  audit({ action: 'browser.disconnect', actorKey: getKey(req), targetType: 'browser', targetId: browserId,
    meta: { clientType: browser.clientType, provider: browser.provider }, req });
  res.json({ ok: true });
});

// ─── Personas ────────────────────────────────────────────────────────────────
//
// A persona is one identity — fingerprint, cookie jar and proxy bound together
// and stable. Rotation means choosing a different persona, never giving one a
// new fingerprint.

router.get('/personas', authMiddleware, (req, res) => {
  res.json({ personas: personas.list(getKey(req)).map(personas.describe) });
});

/** What the creation form may choose, so it never offers a timezone a platform cannot have. */
router.get('/personas/options', authMiddleware, (req, res) => {
  res.json(PREF_OPTIONS);
});

/** The fingerprint these choices would produce. Persists nothing. */
router.post('/personas/preview', authMiddleware, (req, res) => {
  const invalid = personas.prefsError(req.body?.prefs);
  if (invalid) return res.status(400).json({ error: invalid });
  res.json({ fingerprint: personas.describeProfile(personas.preview(req.body?.prefs)) });
});

router.post('/personas', authMiddleware, (req, res) => {
  // Refuse rather than substitute: a persona is a device, and it must be the one asked for.
  const invalid = personas.prefsError(req.body?.prefs);
  if (invalid) return res.status(400).json({ error: invalid });
  const created = personas.create(getKey(req), {
    name: req.body?.name,
    proxy: req.body?.proxy,
    maxConcurrent: req.body?.maxConcurrent,
    prefs: req.body?.prefs,
  });
  audit({ action: 'persona.create', actorKey: getKey(req), targetType: 'persona', targetId: created.id,
    meta: { name: created.name, prefs: created.prefs }, req });
  res.status(201).json(personas.describe(created));
});

/**
 * Name, cap and proxy hint only. The device itself — seed and prefs — is not
 * editable, and a body that tries is refused rather than silently trimmed.
 */
router.put('/personas/:id', authMiddleware, (req, res) => {
  const body = req.body || {};
  const frozen = ['seed', 'prefs', 'id', 'owner', 'isDefault', 'fingerprint'].filter((k) => k in body);
  if (frozen.length) {
    return res.status(400).json({
      error: `${frozen.join(', ')} cannot change after creation — a persona's device is stable for its life. Clone it for a different device.`,
    });
  }
  const updated = personas.update(getKey(req), req.params.id, {
    name: body.name, maxConcurrent: body.maxConcurrent, proxy: body.proxy,
  });
  if (!updated) return res.status(404).json({ error: 'No such persona' });
  audit({ action: 'persona.update', actorKey: getKey(req), targetType: 'persona', targetId: updated.id,
    meta: { fields: Object.keys(body) }, req });
  res.json(personas.describe(updated));
});

router.post('/personas/:id/clone', authMiddleware, (req, res) => {
  const created = personas.clone(getKey(req), req.params.id, { name: req.body?.name });
  if (!created) return res.status(404).json({ error: 'No such persona' });
  audit({ action: 'persona.create', actorKey: getKey(req), targetType: 'persona', targetId: created.id,
    meta: { name: created.name, clonedFrom: req.params.id }, req });
  res.status(201).json(personas.describe(created));
});

/** Pin a persona to one proxy (`{proxyId}`), or unpin it (`{proxyId: null}`). */
router.put('/personas/:id/proxy', authMiddleware, (req, res) => {
  const p = personas.get(getKey(req), req.params.id);
  if (!p) return res.status(404).json({ error: 'No such persona' });
  const proxyId = req.body?.proxyId ?? null;
  if (proxyId === null) {
    proxies.unassign(p.id);
  } else if (!proxies.assign(ownerScope(req), p.id, String(proxyId))) {
    return res.status(404).json({ error: 'No such proxy' });
  }
  audit({ action: 'persona.proxy', actorKey: getKey(req), targetType: 'persona', targetId: p.id,
    meta: { proxyId }, req });
  res.json({ ok: true, proxy: proxies.assigned(p.id)?.toJSON() ?? null });
});

router.get('/personas/:id', authMiddleware, (req, res) => {
  const p = personas.get(getKey(req), req.params.id);
  if (!p) return res.status(404).json({ error: 'No such persona' });
  res.json(personas.describe(p));
});

router.delete('/personas/:id', authMiddleware, (req, res) => {
  try {
    const removed = personas.remove(getKey(req), req.params.id);
    if (!removed) return res.status(404).json({ error: 'No such persona' });
    audit({ action: 'persona.delete', actorKey: getKey(req), targetType: 'persona', targetId: req.params.id, req });
    res.json({ ok: true });
  } catch (err) {
    res.status(err.status || 500).json({ error: err.message });
  }
});

// ─── Desktop pairing ─────────────────────────────────────────────────────────
//
// The `oya://` link the dashboard builds carries one of these codes, never the
// API key. See pairing.js for why.

router.post('/pairing', authMiddleware, enforce('provision'), (req, res) => {
  const key = getKey(req);
  try {
    const persona = personas.resolve(key, req.body?.profile || req.body?.persona);
    const { code, expiresAt } = pairing.issue(key, persona.id);
    audit({ action: 'pairing.issue', actorKey: key, targetType: 'key', targetId: fingerprint(key), req });
    res.status(201).json({ code, expiresAt });
  } catch (err) {
    res.status(err.status || 500).json({ error: err.message });
  }
});

/**
 * Redeem a code for the key it stands for. Unauthenticated by necessity — the
 * desktop app has no credential yet, which is the whole point — so the code is
 * 256 bits, single use, and short-lived, and the attempt is rate limited by IP.
 */
router.post('/pairing/claim', (req, res) => {
  const from = req.ip || req.socket?.remoteAddress || 'unknown';
  if (!consume('connect', `pair:${from}`).allowed) {
    return res.status(429).json({ error: 'Too many pairing attempts' });
  }
  const paired = pairing.claimDetails(req.body?.code);
  if (!paired) return res.status(404).json({ error: 'That pairing code is invalid, used or expired' });
  const { apiKey, persona } = paired;
  audit({ action: 'pairing.claim', actorKey: apiKey, targetType: 'key', targetId: fingerprint(apiKey), req });
  res.json({ apiKey, persona });
});

// ─── Start a browser ─────────────────────────────────────────────────────────

/**
 * The one endpoint a developer needs. Which provider runs the browser is
 * configuration, not the caller's problem — /browsers/connect and
 * /browsers/provision remain as the explicit escape hatches.
 */
export async function startBrowser(req, res) {
  const key = getKey(req);
  const wanted = String(req.body?.provider || keyConfig.providerFor(key));

  let persona;
  try {
    persona = personas.resolve(key, req.body?.profile || req.body?.persona);
  } catch (err) {
    return res.status(err.status || 400).json({ error: err.message });
  }

  const mine = [...registry.browsers.values()].filter((b) => b.apiKey === key).length;
  const quota = checkQuota('browsers', key, mine);
  if (!quota.allowed) return res.status(429).json({ error: `Browser quota reached (${quota.quota})`, ...quota });

  const done = (body) => {
    audit({ action: 'browser.start', actorKey: key, targetType: 'browser', targetId: body.id,
      meta: { provider: body.provider, persona: persona.id }, req });
    res.status(201).json(body);
  };

  try {
    await control().assertProvisioning(key, req.controlSession.id);
    if (wanted === 'oya-selfhosted' && (managedConfigured() || req.controlSession?.runtimeRequired || req.controlSession?.policies?.length || req.body?.governed)) {
      const created = await createManaged({ apiKey: key, browserId: req.controlSession.id, persona: persona.id, name: req.body?.name, policies: req.controlSession.policies });
      return done({ id: created.browserId, provider: wanted, persona: persona.id, status: 'starting', effective: created.runtime });
    }
    // Oya browsers dial in on their own once the sandbox is up.
    if (wanted === 'oya-cloud' || wanted === 'oya-selfhosted') {
      if (!sandboxConfigured()) {
        const missing = missingSettings();
        return res.status(409).json({
          error: `Cloud browsers need ${missing.join(', ')}, which ${missing.length > 1 ? 'are' : 'is'} not set.`
            + (missing.includes('OYA_PUBLIC_WS_URL')
              // The sandbox dials back to this server, so a localhost address
              // is unreachable from a cloud VM.
              ? ' OYA_PUBLIC_WS_URL must be reachable from the sandbox, so a server on'
                + ' localhost needs a tunnel (ngrok, cloudflared) rather than ws://localhost.'
              : ''),
          missing,
        });
      }
      const created = await createSandbox({ apiKey: key, name: req.body?.name, persona: persona.id, browserId: req.controlSession.id });
      return done({
        id: created.browserId, provider: wanted, persona: persona.id, status: 'starting',
        note: 'The browser connects on its own; it appears in GET /browsers within ~90s.',
      });
    }

    // Everything else is a CDP browser we dial out to.
    const wsUrl = req.body?.wsUrl || keyConfig.envFor(key).OYA_CDP_WS_URL;
    if (wanted === 'cdp' && wsUrl) await assertSafeTarget(wsUrl, { label: 'wsUrl' });
    const session = await acquireBrowser({
      provider: wanted, wsUrl, env: keyConfig.envFor(key),
      onCreated: cleanup => control().update(key, req.controlSession.id, { cleanup }),
    });
    const browserId = req.controlSession?.id || uuidv4();
    if (session.cleanup) await control().update(key, browserId, { cleanup: session.cleanup });
    await control().assertProvisioning(key, browserId);
    // The concurrency slot is taken before the vendor session is driven, so a
    // capped persona does not leave a paid-for browser running with nothing
    // holding it.
    try {
      personas.acquire(persona, browserId);
    } catch (capped) {
      await session.release().catch((err) => console.error('[providers] cleanup:', err.message));
      throw capped;
    }
    let driver;
    try {
      driver = await new CDPDriver({
        wsUrl: session.wsUrl,
        provider: session.provider,
        fingerprint: personas.fingerprintFor(persona),
        login: { cookies: getAllCookies(persona.id), origins: structuredClone(getStorage(persona.id)) },
        onStorage: (values) => mergeStorage(persona.id, values),
        onClose: () => { if (registry.get(browserId)) registry.remove(browserId); },
      }).connect();
    } catch (connectErr) {
      personas.release(persona, browserId);
      await session.release().catch((err) => console.error('[providers] cleanup:', err.message));
      throw connectErr;
    }

    registry.add(browserId, {
      apiKey: key,
      name: (req.body?.name || `${session.provider} browser`).slice(0, 100),
      driver, clientType: 'cdp', provider: session.provider, persona,
      release: async () => { await session.release(); personas.release(persona, browserId); },
    });

    usage.browserConnected(key, browserId);
    metrics.browsersConnected.set({}, registry.browsers.size);

    return done({
      id: browserId, provider: session.provider, persona: persona.id, status: 'ready',
      // Our gateway URL, not the vendor's: an agent handed this gets routing,
      // profiles and recording without knowing any of that exists.
      cdpUrl: await browserCdpUrl(req, browserId),
    });
  } catch (err) {
    audit({ action: 'browser.start', actorKey: key, outcome: 'error', meta: { provider: wanted, error: err.message }, req });
    res.status(err.status || 502).json({ error: err.message });
  }
}
router.post('/browsers/start', authMiddleware, enforce('provision'), admission(), startBrowser);


// ─── Challenges: CAPTCHA and MFA ─────────────────────────────────────────────

/** Run a script in a browser we control, whichever kind it is. */
async function evaluateIn(browserId, expression) {
  const result = await sendCommand(browserId, 'evaluate_raw', { expression });
  return result?.data?.result ?? result?.data ?? null;
}

// Anchor, Browserbase, Steel and Browser Use solve natively; solving again pays
// twice and can race their own attempt.
const NATIVE_CAPTCHA = ['anchor', 'browserbase', 'steel', 'browseruse'];

/** Mirrors MAX_FILE_BYTES in the SDK's file(); express.json()'s limit is sized for it. */
export const MAX_FILE_BYTES = 10 * 1024 * 1024;

/** A task file from the SDK's `file()`. Checked here because this is the trust boundary. */
const validFile = (v) => !!v && typeof v === 'object' && !Array.isArray(v)
  && typeof v.file === 'string' && v.file.length > 0 && v.file.length <= 255
  && typeof v.type === 'string' && v.type.length > 0 && v.type.length <= 128
  && typeof v.b64 === 'string' && /^[A-Za-z0-9+/]*={0,2}$/.test(v.b64)
  && v.b64.length <= Math.ceil(MAX_FILE_BYTES / 3) * 4;

/**
 * Agent data and playbook variables: names to strings, numbers, or a file.
 * `secrets` passes `{ files: false }` — a file is never typed through a placeholder, so
 * redact() cannot hide one and accepting it would be a silent downgrade, not a secret.
 */
export const validData = (d, { files = true } = {}) => !!d && typeof d === 'object' && !Array.isArray(d)
  && Object.entries(d).every(([k, v]) => /^\w{1,64}$/.test(k)
    && (['string', 'number'].includes(typeof v) || (files && validFile(v))));

/**
 * Between page-changing steps of a run: clear a CAPTCHA or MFA prompt, or park the
 * run on a person. ponytail: two detection evals per page-changing step.
 */
function checkpointFor(apiKey, browserId, requestHuman) {
  const liveViewUrl = `/dashboard/?browser=${encodeURIComponent(browserId)}`;
  const evaluate = (expr) => evaluateIn(browserId, expr);
  return async () => {
    const browser = registry.get(browserId);
    const c = await captcha.handle(evaluate, { providerSolves: NATIVE_CAPTCHA.includes(browser?.provider), env: keyConfig.envFor(apiKey) }).catch(() => null);
    if (c?.present && !c.solved && !c.invisible && c.method !== 'provider') {
      await requestHuman({ reason: 'captcha', message: c.error || 'A CAPTCHA needs solving. Solve it in the live view, then respond.', liveViewUrl });
    }
    const m = await mfa.complete(evaluate, browser?.persona?.id || personas.defaultFor(apiKey).id, { liveViewUrl }).catch(() => null);
    if (m?.present && !m.completed) {
      await requestHuman({ reason: 'mfa', message: m.error || 'MFA needs completing in the live view.', liveViewUrl });
    }
  };
}

router.post('/browsers/:browserId/captcha', authMiddleware, enforce('command'), async (req, res) => {
  const { browserId } = req.params;
  if (!registry.isConnected(browserId) || !canAccess(req, browserId)) {
    return res.status(404).json({ error: `Browser ${browserId} not connected` });
  }
  const browser = registry.get(browserId);
  try {
    const result = await captcha.handle((expr) => evaluateIn(browserId, expr), {
      solve: req.body?.solve !== false,
      providerSolves: NATIVE_CAPTCHA.includes(browser.provider),
      env: keyConfig.envFor(getKey(req)),
    });
    if (result.present) {
      audit({ action: 'captcha.handle', actorKey: getKey(req), targetType: 'browser', targetId: browserId,
        outcome: result.solved ? 'ok' : 'error', meta: { type: result.type, method: result.method }, req });
    }
    res.json(result);
  } catch (err) {
    res.status(err.status || 500).json({ error: err.message });
  }
});

router.post('/browsers/:browserId/mfa', authMiddleware, enforce('command'), async (req, res) => {
  const { browserId } = req.params;
  if (!registry.isConnected(browserId) || !canAccess(req, browserId)) {
    return res.status(404).json({ error: `Browser ${browserId} not connected` });
  }
  const browser = registry.get(browserId);
  const personaId = browser.persona?.id || personas.defaultFor(getKey(req)).id;
  try {
    const result = await mfa.complete((expr) => evaluateIn(browserId, expr), personaId, {
      liveViewUrl: `/dashboard/?browser=${encodeURIComponent(browserId)}`,
    });
    if (result.present) {
      audit({ action: 'mfa.complete', actorKey: getKey(req), targetType: 'browser', targetId: browserId,
        outcome: result.completed ? 'ok' : 'error', meta: { method: result.method }, req });
    }
    res.json(result);
  } catch (err) {
    res.status(err.status || 500).json({ error: err.message });
  }
});

/** Configure a persona's second factor. The secret is write-only. */
router.put('/personas/:id/mfa', authMiddleware, async (req, res) => {
  const p = personas.get(getKey(req), req.params.id);
  if (!p) return res.status(404).json({ error: 'No such persona' });
  try {
    const described = await mfa.set(p.id, req.body);
    audit({ action: 'mfa.configure', actorKey: getKey(req), targetType: 'persona', targetId: p.id,
      meta: { type: described.type }, req });
    res.json(described);
  } catch (err) {
    res.status(err.status || 500).json({ error: err.message });
  }
});

router.delete('/personas/:id/mfa', authMiddleware, (req, res) => {
  const p = personas.get(getKey(req), req.params.id);
  if (!p) return res.status(404).json({ error: 'No such persona' });
  mfa.clear(p.id);
  audit({ action: 'mfa.clear', actorKey: getKey(req), targetType: 'persona', targetId: p.id, req });
  res.json({ ok: true });
});

// ─── Proxies ─────────────────────────────────────────────────────────────────
//
// A proxy is part of a persona's identity, assigned once and kept — an exit IP
// that changes mid-life looks like an account takeover.

router.get('/proxies', authMiddleware, (req, res) => {
  res.json({ proxies: proxies.list(fingerprint(getKey(req))) });
});

router.post('/proxies', authMiddleware, async (req, res) => {
  try {
    const created = await proxies.register({
      owner: fingerprint(getKey(req)),
      label: req.body?.label,
      url: req.body?.url,
      geo: req.body?.geo,
      kind: req.body?.kind,
      maxPersonas: req.body?.maxPersonas,
    });
    audit({ action: 'proxy.create', actorKey: getKey(req), targetType: 'proxy', targetId: created.id,
      meta: { geo: created.geo, kind: created.kind }, req });
    res.status(201).json(created.toJSON());
  } catch (err) {
    res.status(err.status || 500).json({ error: err.message });
  }
});

router.delete('/proxies/:id', authMiddleware, (req, res) => {
  try {
    const removed = proxies.remove(fingerprint(getKey(req)), req.params.id);
    if (!removed) return res.status(404).json({ error: 'No such proxy' });
    audit({ action: 'proxy.delete', actorKey: getKey(req), targetType: 'proxy', targetId: req.params.id, req });
    res.json({ ok: true });
  } catch (err) {
    res.status(err.status || 500).json({ error: err.message });
  }
});

/** Verify each proxy works and report the address its traffic actually leaves from. */
router.post('/proxies/check', authMiddleware, async (req, res) => {
  const results = await proxies.checkAll(fingerprint(getKey(req)));
  res.json({ results });
});

// ─── CDP gateway: sessions, providers, profiles, recordings ──────────────────

router.get('/gateway/sessions', authMiddleware, (req, res) => {
  const key = getKey(req);
  res.json({ sessions: listSessions(key) });
});

router.delete('/gateway/sessions/:id', authMiddleware, async (req, res) => {
  const session = gatewaySessions.get(req.params.id);
  if (!session) return res.status(404).json({ error: 'No such session' });
  if (session.apiKey !== getKey(req)) {
    return res.status(404).json({ error: 'No such session' });
  }
  await killSession(req.params.id, req.body?.reason || 'closed by operator');
  audit({ action: 'gateway.session.kill', actorKey: getKey(req), targetType: 'session', targetId: req.params.id, req });
  res.json({ ok: true });
});

/** Provider pool: health, capacity, latency and the queue. */
router.get('/gateway/providers', authMiddleware, (req, res) =>
  res.json(pool.stats(fingerprint(getKey(req)))));

router.post('/gateway/providers', authMiddleware, async (req, res) => {
  try {
    const key = getKey(req);
    const cfg = validateProviderConfig({ ...(req.body || {}), owner: fingerprint(key) });
    const env = keyConfig.envFor(key);
    const credentialField = `${cfg.type}_api_key`;
    const credential = typeof req.body?.apiKey === 'string' ? req.body.apiKey.trim() : '';
    if (credential && keyConfig.FIELDS[credentialField]?.secret) env[`${cfg.type.toUpperCase()}_API_KEY`] = credential;
    const supported = availableProviders(env).find(p => p.name === cfg.type);
    if (!supported) return res.status(400).json({ error: 'Unknown browser provider.' });
    if (!supported.configured) return res.status(409).json({ error: 'Add an API key for this provider, or save one in Settings → Browsers.' });
    if (pool.get(cfg.owner, cfg.name)) return res.status(409).json({ error: 'A provider with this name already exists. Choose another name.' });
    // The host dials this URL, using its network position rather than the
    // caller's. Unvalidated, that is a server-side request forgery primitive.
    if (cfg.wsUrl) {
      const safe = await assertSafeTarget(cfg.wsUrl, { label: 'wsUrl' });
      cfg.wsUrl = safe.href;
    }
    // Recheck after DNS validation, which can yield to a concurrent addition.
    if (pool.get(cfg.owner, cfg.name)) return res.status(409).json({ error: 'A provider with this name already exists. Choose another name.' });
    const provider = pool.register(cfg);
    if (credential && keyConfig.FIELDS[credentialField]?.secret) await keyConfig.set(key, { [credentialField]: credential });
    await keyConfig.saveRouting(key, pool);
    audit({ action: 'provider.upsert', actorKey: getKey(req), targetType: 'provider', targetId: provider.name,
      meta: { type: provider.type, maxConcurrent: provider.maxConcurrent, priority: provider.priority }, req });
    res.json(provider.toJSON());
  } catch (err) {
    res.status(err.status || 400).json({ error: err.message });
  }
});

router.delete('/gateway/providers/:name', authMiddleware, async (req, res) => {
  try {
    // Only your own; shared host providers are not yours to remove.
    const removed = pool.remove(fingerprint(getKey(req)), req.params.name);
    if (removed) audit({ action: 'provider.remove', actorKey: getKey(req), targetType: 'provider', targetId: req.params.name, req });
    if (removed) await keyConfig.saveRouting(getKey(req), pool);
    res.status(removed ? 200 : 404).json(removed ? { ok: true } : { error: 'Provider not found.' });
  } catch (err) { res.status(err.status || 500).json({ error: err.message }); }
});

router.post('/gateway/strategy', authMiddleware, async (req, res) => {
  const strategy = String(req.body?.strategy || '');
  if (!STRATEGIES.includes(strategy)) {
    return res.status(400).json({ error: `strategy must be one of ${STRATEGIES.join(', ')}` });
  }
  // Per key: how you want your sessions routed is not a host-wide switch.
  const owner = fingerprint(getKey(req));
  const previous = pool.strategyFor(owner);
  pool.setStrategy(owner, strategy);
  await keyConfig.saveRouting(getKey(req), pool);
  audit({ action: 'routing.strategy', actorKey: getKey(req), targetType: 'routing',
    meta: { from: previous, to: strategy }, req });
  res.json({ ok: true, strategy });
});

// Profiles and recordings are per-key. Names and session ids are caller-chosen
// or guessable, so they are scoped by owner rather than treated as secrets.
router.get('/gateway/profiles', authMiddleware, async (req, res) =>
  res.json({ profiles: await profiles.list(fingerprint(getKey(req))) }));

router.delete('/gateway/profiles/:name', authMiddleware, async (req, res) => {
  try {
    const removed = await profiles.remove(fingerprint(getKey(req)), req.params.name);
    audit({ action: 'profile.delete', actorKey: getKey(req), targetType: 'profile', targetId: req.params.name,
      outcome: removed ? 'ok' : 'error', req });
    res.json({ ok: removed });
  } catch (err) {
    res.status(err.status || 500).json({ error: err.message });
  }
});

router.get('/gateway/recordings', authMiddleware, async (req, res) =>
  res.json({ recordings: await recorder.list(fingerprint(getKey(req))) }));

router.get('/gateway/recordings/:id', authMiddleware, async (req, res) => {
  const found = await recorder.manifest(req.params.id, ownerScope(req));
  if (!found) return res.status(404).json({ error: 'No such recording' });
  res.json(found);
});

/** One frame of a recording, for the dashboard player to scrub through. */
router.get('/gateway/recordings/:id/frames/:index', authMiddleware, async (req, res) => {
  const buf = await recorder.frame(req.params.id, req.params.index, ownerScope(req));
  if (!buf) return res.status(404).json({ error: 'No such frame' });
  res.type('image/jpeg').set('Cache-Control', 'private, max-age=3600').send(buf);
});

router.delete('/gateway/recordings/:id', authMiddleware, async (req, res) => {
  const removed = await recorder.remove(req.params.id, ownerScope(req));
  audit({ action: 'recording.delete', actorKey: getKey(req), targetType: 'recording', targetId: req.params.id, req });
  res.json({ ok: removed });
});

// ─── Oya Cloud browser provisioning ───
//
// Launches sandboxed browsers that enroll over the normal WebSocket with the
// caller's own key, so they join that caller's pool as ordinary browsers.

router.post('/browsers/provision', authMiddleware, enforce('provision'), async (req, res) => {
  if (registry.draining) return res.status(503).json({ error: 'Server is draining' });
  const hourly = checkHourly('sandboxesPerHour', getKey(req));
  if (!hourly.allowed) {
    audit({ action: 'browser.provision', actorKey: getKey(req), outcome: 'denied',
      meta: { reason: 'hourly quota', quota: hourly.quota }, req });
    return res.status(429).json({ error: `Sandbox quota reached for this hour (${hourly.quota})`, ...hourly });
  }
  if (!sandboxConfigured()) {
    const missing = missingSettings();
    return res.status(409).json({
      error: `Cloud browsers need ${missing.join(', ')}, which ${missing.length > 1 ? 'are' : 'is'} not set.`
        + (missing.includes('OYA_PUBLIC_WS_URL')
          ? ' OYA_PUBLIC_WS_URL must be reachable from the sandbox, so a server on'
            + ' localhost needs a tunnel (ngrok, cloudflared) rather than ws://localhost.'
          : ''),
      missing,
    });
  }
  const key = getKey(req);
  const count = Math.min(Math.max(parseInt(req.body?.count) || 1, 1), 100);
  const name = typeof req.body?.name === 'string' ? req.body.name.slice(0, 100) : undefined;

  // Bounded concurrency: a fleet is built by repeating this call, and firing
  // every create at once would just rate-limit us at the provider.
  const IN_FLIGHT = 10;
  const results = [];
  const queue = Array.from({ length: count }, (_, i) => i);
  await Promise.all(Array.from({ length: Math.min(IN_FLIGHT, count) }, async () => {
    while (queue.length) {
      queue.shift();
      try { results.push({ status: 'fulfilled', value: await createSandbox({ apiKey: key, name }) }); }
      catch (reason) { results.push({ status: 'rejected', reason }); }
    }
  }));
  const created = results.filter((r) => r.status === 'fulfilled').map((r) => r.value);
  const failed = results.filter((r) => r.status === 'rejected').map((r) => r.reason?.message || 'unknown error');

  if (failed.length) console.error(`[sandbox] ${failed.length}/${count} failed:`, failed.join('; '));
  usage.record(key, 'sandboxes_created', created.length);
  metrics.sandboxes.inc({ op: 'create', outcome: 'ok' }, created.length);
  if (failed.length) metrics.sandboxes.inc({ op: 'create', outcome: 'error' }, failed.length);
  audit({ action: 'browser.provision', actorKey: key, targetType: 'sandbox',
    outcome: created.length ? 'ok' : 'error',
    meta: { requested: count, created: created.length, failed: failed.length }, req });

  res.status(created.length ? 202 : 502).json({
    ok: created.length > 0,
    requested: count,
    browsers: created,
    failed,
    note: 'Browsers connect on their own; they appear in GET /browsers within ~90s.',
  });
});

router.delete('/browsers/:browserId/sandbox', authMiddleware, async (req, res) => {
  const { browserId } = req.params;
  try {
    // Ownership is enforced by the sandbox's owner label, which works whether or
    // not the browser is currently connected.
    const removed = await removeSandbox(browserId, getKey(req));
    metrics.sandboxes.inc({ op: 'delete', outcome: 'ok' });
    audit({ action: 'sandbox.delete', actorKey: getKey(req), targetType: 'sandbox', targetId: browserId,
      meta: { removed }, req });
    res.json({ ok: true, removed });
  } catch (err) {
    console.error('[sandbox] delete failed:', err.message);
    res.status(err.status || 502).json({ error: err.message });
  }
});

// Runtime config — server-wide settings (OpenAI key, model, base URL).
// Admin only: anyone who can write this can hijack every tenant's chat requests.
// Settings belong to the API key that presents them — the same identity that
// owns the browsers, personas and cookies. Nothing is inherited from an
// account, and nothing has to be in the environment; a key that has set
// nothing falls back to the host defaults.
router.get('/config', authMiddleware, (req, res) => {
  try {
    res.json(keyConfig.get(getKey(req)));
  } catch (err) {
    res.status(err.status || 500).json({ error: err.message });
  }
});

router.post('/config', authMiddleware, async (req, res) => {
  const key = getKey(req);
  try {
    await keyConfig.set(key, req.body);
    audit({ action: 'config.update', actorKey: key, targetType: 'config',
      meta: { fields: Object.keys(req.body || {}) }, req });
    res.json({ ok: true, ...keyConfig.get(key) });
  } catch (err) {
    res.status(err.status || 500).json({ error: err.message });
  }
});

// Writing the deployment-wide default affects every key that has not set its
// own, so it stays behind the operator token.
router.post('/config/host', operatorOnly, async (req, res) => {
  try {
    await runtimeConfig.set(req.body);
    audit({ action: 'config.update', actorKey: getKey(req), targetType: 'config',
      meta: { scope: 'host', fields: Object.keys(req.body || {}) }, req });
    res.json({ ok: true, scope: 'host', ...runtimeConfig.get() });
  } catch (err) {
    res.status(err.status || 500).json({ error: err.message });
  }
});

// List connected browsers — scoped to caller's API key (admin sees all)
router.get('/browsers', authMiddleware, async (req, res) => {
  const key = getKey(req);
  res.json(await listSandboxBrowsers(key, registry.list(key)));
});

// Live view — SSE stream of JPEG frames.
//
// Header auth, or ?ticket= for EventSource, which cannot set headers. A ticket
// is single-use and lives 60 seconds (control().ticket / redeem, redeemed in
// forwardHttp). ?key= used to be accepted here: that put a project's permanent
// administrator credential into browser history, Referer headers, proxy and
// CDN access logs, and — via `oya open` — the process argv table.
router.get('/live/:browserId', authMiddleware, (req, res) => {
  const { browserId } = req.params;

  if (!registry.isConnected(browserId) || !canAccess(req, browserId)) {
    return res.status(404).json({ error: `Browser ${browserId} not connected` });
  }

  res.writeHead(200, {
    'Content-Type': 'text/event-stream',
    'Cache-Control': 'no-cache',
    Connection: 'keep-alive',
  });

  // Send latest frame immediately if available
  const browser = registry.get(browserId);
  if (browser?.lastFrame) {
    try { res.write(`data: ${browser.lastFrame}\n\n`); } catch {}
  }

  res.authToken = req.authToken;
  registry.addViewer(browserId, res);

  req.on('close', () => {
    registry.removeViewer(browserId, res);
  });
});

// Send command to a browser
/**
 * Actions the server sends on its own behalf and a caller may not.
 *
 * `evaluate_raw` runs arbitrary JavaScript in the page's own world, which is
 * how CAPTCHA and MFA handling reach a site's globals. Exposed here it would
 * be arbitrary code execution inside a browser holding the customer's real
 * cookies and logged-in sessions — the CDP driver already refuses it, and the
 * Oya client must not be the way around that.
 */
const INTERNAL_ACTIONS = new Set(['evaluate_raw', 'evaluate', 'record']);

router.post('/browsers/:browserId/command', authMiddleware, enforce('command'), async (req, res) => {
  // Navigate can take up to 90s — disable socket timeout for this request
  req.setTimeout(0);
  res.setTimeout(0);

  const { browserId } = req.params;
  const { action, params } = req.body;

  if (!action) {
    return res.status(400).json({ error: 'Missing action' });
  }
  if (INTERNAL_ACTIONS.has(action)) {
    return res.status(403).json({ error: `${action} is not available through this API` });
  }

  if (!registry.isConnected(browserId) || !canAccess(req, browserId)) {
    return res.status(404).json({ error: `Browser ${browserId} not connected` });
  }

  try {
    const result = await sendCommand(browserId, action, params || {});
    // A recording in progress keeps the navigations the person asked for here; what
    // they click and type is seen in the page itself.
    if (result?.ok !== false) flow.noteCommand(browserId, action, params || {});
    usage.record(getKey(req), 'commands');
    if (result?.ok === false) usage.record(getKey(req), 'command_errors');
    res.json(result);
  } catch (err) {
    usage.record(getKey(req), 'command_errors');
    res.status(err.status || 500).json({ ok: false, error: err.message, ...(err.code ? { code: err.code } : {}) });
  }
});

// Chat — LLM + MCP tools for natural-language browser control
router.post('/browsers/:browserId/chat', authMiddleware, enforce('chat'), async (req, res) => {
  req.setTimeout(0);
  res.setTimeout(0);

  const { browserId } = req.params;
  const { messages, data = {}, secrets = {} } = req.body;

  if (!messages || !Array.isArray(messages)) {
    return res.status(400).json({ error: 'messages array required' });
  }
  if (!validData(data) || !validData(secrets, { files: false })) return res.status(400).json({ error: 'data must map names to strings, numbers or a file() value; secrets takes strings and numbers only' });

  if (!registry.isConnected(browserId) || !canAccess(req, browserId)) {
    return res.status(404).json({ error: `Browser ${browserId} not connected` });
  }

  const toolCalls = [];
  await longJson(res, async () => {
    const result = await runChat(browserId, messages, {
      apiKey: getKey(req),
      data,
      secrets,
      onToolCall: ({ name, args }) => toolCalls.push({ name, args }),
      onText: () => {},
    });
    return { text: result.text, toolCalls };
  });
});

/**
 * Agent work can outlast Node fetch's 300s headers timeout (and proxy idle
 * timeouts), so commit to 200 now and trickle whitespace; JSON.parse ignores it.
 * Errors after this point can only travel in the body.
 */
async function longJson(res, work) {
  res.writeHead(200, { 'Content-Type': 'application/json' });
  const keepalive = setInterval(() => res.write(' '), 15_000);
  try {
    res.end(JSON.stringify(await work()));
  } catch (err) {
    // The 200 is already sent, so the real status (429 for a quota) travels in the body.
    res.end(JSON.stringify({ error: err.message, status: err.status || 500 }));
  } finally {
    clearInterval(keepalive);
  }
}

// Playbooks — save this browser's last ask() by name, then replay it without the LLM.
// With `steps` in the body, the browser recorded a person doing the task instead:
// same schema, same storage, same Playwright export.
router.post('/browsers/:browserId/playbooks', authMiddleware, enforce('chat'), async (req, res) => {
  const { browserId } = req.params;
  if (!registry.isConnected(browserId) || !canAccess(req, browserId)) {
    return res.status(404).json({ error: `Browser ${browserId} not connected` });
  }
  try {
    const { name, steps, prompt, secrets, schemaVersion, variables } = req.body || {};
    if (schemaVersion !== undefined && schemaVersion !== 2) return res.status(400).json({ error: 'Unsupported workflow version' });
    let run;
    if (steps !== undefined) {
      if (secrets !== undefined && (!Array.isArray(secrets) || secrets.some((k) => !/^\w{1,64}$/.test(String(k))))) {
        return res.status(400).json({ error: 'secrets must be an array of variable names' });
      }
      run = {
        prompt: String(prompt || name || '').slice(0, 2000),
        steps: schemaVersion === 2 ? playbooks.validateWorkflow({ schemaVersion, steps, variables, secrets }).steps : playbooks.sanitizeSteps(steps),
        ...(schemaVersion === 2 ? { schemaVersion, variables: playbooks.validateWorkflow({ schemaVersion, steps, variables, secrets }).variables } : {}),
        secrets: (secrets || []).map(String),
      };
    }
    res.json(await playbooks.create(getKey(req), browserId, name, run));
  } catch (err) {
    res.status(err.status || 500).json({ error: err.message });
  }
});

router.post('/browsers/:browserId/playbooks/:name/play', authMiddleware, enforce('chat'), async (req, res) => {
  req.setTimeout(0);
  res.setTimeout(0);

  const { browserId, name } = req.params;
  const variables = req.body?.variables ?? {};
  if (!registry.isConnected(browserId) || !canAccess(req, browserId)) {
    return res.status(404).json({ error: `Browser ${browserId} not connected` });
  }
  if (!validData(variables)) return res.status(400).json({ error: 'variables must map names to strings, numbers or a file() value' });
  const pb = keyConfig.getPlaybook(getKey(req), name);
  if (!pb) return res.status(404).json({ error: `No playbook named ${name}` });
  const missing = playbooks.missingVariables(pb, variables);
  if (missing.length) return res.status(400).json({ error: `Missing variables: ${missing.join(', ')}` });

  await longJson(res, () => playbooks.play(getKey(req), browserId, pb, variables, { autoHeal: req.body?.autoHeal !== false }));
});

router.get('/playbooks', authMiddleware, (req, res) => {
  res.json({ playbooks: playbooks.list(getKey(req)) });
});

router.delete('/playbooks/:name', authMiddleware, async (req, res) => {
  try {
    await playbooks.remove(getKey(req), req.params.name);
    res.json({ ok: true });
  } catch (err) {
    res.status(err.status || 500).json({ error: err.message });
  }
});

router.post('/playbooks/:name/promote', authMiddleware, async (req, res) => {
  try {
    res.json(await playbooks.promote(getKey(req), req.params.name));
  } catch (err) {
    res.status(err.status || 500).json({ error: err.message });
  }
});

// Runs — submit a prompt or playbook in the background; the SDK polls and fires callbacks
router.post('/browsers/:browserId/runs', authMiddleware, enforce('chat'), async (req, res) => {
  const { browserId } = req.params;
  const { prompt, playbook, data = {}, secrets = {}, autoHeal = true } = req.body || {};
  const key = getKey(req);
  if (!registry.isConnected(browserId) || !canAccess(req, browserId)) {
    return res.status(404).json({ error: `Browser ${browserId} not connected` });
  }
  if (!validData(data) || !validData(secrets, { files: false })) return res.status(400).json({ error: 'data must map names to strings, numbers or a file() value; secrets takes strings and numbers only' });
  if ((typeof prompt === 'string') === (typeof playbook === 'string')) {
    return res.status(400).json({ error: 'Pass exactly one of prompt or playbook' });
  }
  const pb = playbook ? keyConfig.getPlaybook(key, playbook) : null;
  if (playbook && !pb) return res.status(404).json({ error: `No playbook named ${playbook}` });
  const missing = pb ? playbooks.missingVariables(pb, { ...data, ...secrets }) : [];
  if (missing.length) return res.status(400).json({ error: `Missing data: ${missing.join(', ')}` });

  const run = runs.start(key, browserId, async ({ requestHuman }) => {
    const checkpoint = checkpointFor(key, browserId, requestHuman);
    if (pb) return playbooks.play(key, browserId, pb, { ...data, ...secrets }, { autoHeal: autoHeal !== false, checkpoint, requestHuman });
    const result = await runChat(browserId, [{ role: 'user', content: prompt }], { apiKey: key, data, secrets, checkpoint, requestHuman });
    if (result.limited) throw new Error('The agent hit its step limit without finishing');
    // Anywhere in the reply, not just the first line: a model that narrates before
    // its verdict still failed, and a "succeeded" run saves the broken steps as a playbook.
    if (/(^|\n)\s*FAILED:/i.test(result.text)) throw new Error(result.text.trim());
    return { text: result.text };
  });
  res.status(202).json(run);
});

router.get('/runs/:runId', authMiddleware, (req, res) => {
  const run = runs.get(fingerprint(getKey(req)), req.params.runId);
  if (!run) return res.status(404).json({ error: 'No such run' });
  res.json(run);
});

router.post('/runs/:runId/respond', authMiddleware, (req, res) => {
  if (!runs.respond(fingerprint(getKey(req)), req.params.runId, req.body?.response)) {
    return res.status(409).json({ error: 'This run is not waiting for a person' });
  }
  res.json({ ok: true });
});

// ─── Pool Endpoints ─────────────────────────────────────────────────────────

// Pool status — how many browsers, who's connected
router.get('/pool', authMiddleware, (req, res) => {
  const key = getKey(req);
  res.json(poolStats(key));
});

// Pool command — send a command to the next browser via round-robin
router.post('/pool/command', authMiddleware, async (req, res) => {
  req.setTimeout(0);
  res.setTimeout(0);

  const key = getKey(req);
  const { action, params } = req.body;

  if (!action) {
    return res.status(400).json({ error: 'Missing action' });
  }
  // The pool is the same surface by another door.
  if (INTERNAL_ACTIONS.has(action)) {
    return res.status(403).json({ error: `${action} is not available through this API` });
  }

  const browserId = nextBrowser(key);
  if (!browserId) {
    return res.status(503).json({ error: 'No browsers available in pool' });
  }

  try {
    const result = await sendCommand(browserId, action, params || {});
    res.json({ ...result, _browser: browserId });
  } catch (err) {
    res.status(500).json({ ok: false, error: err.message, _browser: browserId });
  }
});

// Pool cookies — view the shared cookie jar for the caller's API key.
// Admin also gets a per-key breakdown.
// Cookies live with the persona, not the key — the jar and the fingerprint
// have to move together or a returning session looks like a new device.
router.get('/pool/cookies', authMiddleware, (req, res) => {
  try {
    const persona = personas.resolve(getKey(req), req.query.persona);
    res.json({ persona: persona.id, cookies: getAllCookies(persona.id) });
  } catch (err) {
    res.status(err.status || 500).json({ error: err.message });
  }
});

// Pool cookies — clear the caller's API-key jar only. Admin clears every jar.
router.delete('/pool/cookies', authMiddleware, (req, res) => {
  const key = getKey(req);
  let persona;
  try {
    persona = personas.resolve(key, req.query.persona);
  } catch (err) {
    return res.status(err.status || 500).json({ error: err.message });
  }
  clearCookies(persona.id);
  // Destroying sessions is exactly the action you want a record of afterwards.
  audit({ action: 'cookies.clear', actorKey: key, targetType: 'cookies', targetId: persona.id, req });
  res.json({ ok: true, persona: persona.id });
});

router.use((err, req, res, next) => {
  if (res.headersSent) return next(err);
  res.status(err.status || 503).json({ error: err.status ? err.message : 'Operation could not be completed', code: err.code || 'operation_failed' });
});
