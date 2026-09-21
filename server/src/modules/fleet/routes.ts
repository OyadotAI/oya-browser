/**
 * REST routes: fleet.
 */

import { Router } from 'express';
import { authMiddleware, provisionKeys } from '../auth/service.ts';
import { registry } from '../browsers/registry.ts';
import { listSandboxBrowsers } from '../../drivers/sandbox.ts';
import { metrics, render as renderMetrics } from '../../platform/metrics.ts';
import { audit, history as auditHistory, fingerprint } from '../../platform/audit.ts';
import * as usage from '../../platform/usage.ts';
import { status as limitStatus, QUOTAS } from '../../platform/limits.ts';
import { listSessions } from '../gateway/service.ts';
import { pool } from '../gateway/routing.ts';
import { control } from '../control/service.ts';
import { getKey, operatorOnly } from '../../app/http.ts';
import { summarize } from './summary.ts';
import { MAX_PROVISION, DEFAULT_USAGE_HOURS, DEFAULT_AUDIT_LIMIT, MAX_AUDIT_LIMIT } from './constants.ts';

/** Health, key provisioning, metrics, fleet overview, usage, audit and operator drain routes. */
export const router = Router({ caseSensitive: true });

/** GET /health, liveness, connected browser count and uptime; no auth required. */
router.get('/health', (req, res) => {
  res.json({
    status: 'ok',
    browsers: registry.list().length,
    uptime: process.uptime(),
  });
});

/**
 * POST /fleet/provision, batch-mint API keys (1–10000); operatorOnly. Minting credentials is a host
 * capability, a tenant key must not be able to mint more identities. Accounts still create
 * their own keys through /auth/keys.
 */
router.post('/fleet/provision', operatorOnly, async (req, res) => {
  const key = getKey(req);
  const count = Math.min(Math.max(parseInt(req.query.count || req.body?.count) || 1, 1), MAX_PROVISION);
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

/** Session totals for the fleet view. */
const sessionCounts = (sessions) => ({
  total: sessions.length,
  attached: sessions.filter((s) => s.connected).length,
  recording: sessions.filter((s) => s.recording).length,
});

/** When the fleet view was taken, and how long this instance has been up. */
const stamp = () => ({ at: new Date().toISOString(), uptimeSeconds: Math.round(process.uptime()) });

/** A key's usage so far and its remaining allowance. */
const allowance = (key) => ({ usage: usage.current(key), ...limitStatus(key) });

/** Builds the fleet view for one key. */
async function fleetView(key) {
  const mine = await listSandboxBrowsers(key, registry.list(key));
  return {
    ...stamp(),
    browsers: summarize(mine),
    sessions: sessionCounts(listSessions(key)),
    routing: pool.stats(fingerprint(key)),
    ...allowance(key),
  };
}

/**
 * Everything this key owns, in one call: its browsers, sessions, providers,
 * usage and allowance. This is the dashboard's fleet view. There is no admin
 * variant, because the key is the whole identity.
 */
router.get('/fleet', authMiddleware, async (req, res) => {
  res.json(await fleetView(getKey(req)));
});

/** The caller's own usage and remaining allowance, no admin key needed. */
router.get('/usage', authMiddleware, async (req, res) => {
  const key = getKey(req);
  const mine = [...registry.browsers.values()].filter((b) => b.apiKey === key).length;
  res.json({
    current: usage.current(key),
    browsers: { connected: mine, quota: QUOTAS.browsers },
    ...limitStatus(key),
    history: (await usage.history(key, { hours: Number(req.query.hours) || DEFAULT_USAGE_HOURS })).rows,
  });
});

/** GET /audit, this key's own audit trail: never another key's, so it needs no administrator. */
router.get('/audit', authMiddleware, async (req, res) => {
  const result = await auditHistory({
    limit: Math.min(Number(req.query.limit) || DEFAULT_AUDIT_LIMIT, MAX_AUDIT_LIMIT),
    action: req.query.action,
    actor: fingerprint(getKey(req)), // never another key's history
    since: req.query.since,
  });
  res.json(result);
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
