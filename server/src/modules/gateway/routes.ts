/**
 * REST routes: gateway.
 */

import { Router } from 'express';
import { authMiddleware } from '../auth/service.ts';
import { audit, fingerprint } from '../../platform/audit.ts';
import { listSessions, killSession, sessions as gatewaySessions } from './service.ts';
import { pool, STRATEGIES } from './routing.ts';
import { registerProvider } from './provider-registration.ts';
import * as profiles from './profiles.ts';
import * as recorder from './recorder.ts';
import * as keyConfig from '../config/service.ts';
import { getKey, ownerScope } from '../../app/http.ts';
import { Status } from '../../platform/http-status.ts';

/** The /gateway routes; every one is scoped to the calling key. */
export const router = Router({ caseSensitive: true });

// ─── CDP gateway: sessions, providers, profiles, recordings ──────────────────

/** GET /gateway/sessions — this key's live CDP gateway sessions. */
router.get('/gateway/sessions', authMiddleware, (req, res) => {
  const key = getKey(req);
  res.json({ sessions: listSessions(key) });
});

/** DELETE /gateway/sessions/:id — kills one of this key's gateway sessions; another key's reads as not found. */
router.delete('/gateway/sessions/:id', authMiddleware, async (req, res) => {
  const session = gatewaySessions.get(req.params.id);
  if (!session) return res.status(Status.NOT_FOUND).json({ error: 'No such session' });
  if (session.apiKey !== getKey(req)) {
    return res.status(Status.NOT_FOUND).json({ error: 'No such session' });
  }
  await killSession(req.params.id, req.body?.reason || 'closed by operator');
  audit({ action: 'gateway.session.kill', actorKey: getKey(req), targetType: 'session', targetId: req.params.id, req });
  res.json({ ok: true });
});

/** Provider pool: health, capacity, latency and the queue. */
router.get('/gateway/providers', authMiddleware, (req, res) => res.json(pool.stats(fingerprint(getKey(req)))));

/** POST /gateway/providers — registers a browser provider for this key, saving its API key when one is supplied. */
router.post('/gateway/providers', authMiddleware, async (req, res) => {
  try {
    const provider = await registerProvider(getKey(req), fingerprint(getKey(req)), req.body);
    auditUpsert(req, provider);
    res.json(provider.toJSON());
  } catch (err) {
    res.status(err.status || Status.BAD_REQUEST).json({ error: err.message });
  }
});

/** Audits a provider registration. */
function auditUpsert(req, provider) {
  audit({
    action: 'provider.upsert',
    actorKey: getKey(req),
    targetType: 'provider',
    targetId: provider.name,
    meta: { type: provider.type, maxConcurrent: provider.maxConcurrent, priority: provider.priority },
    req,
  });
}

/** DELETE /gateway/providers/:name — removes one of this key's providers. */
router.delete('/gateway/providers/:name', authMiddleware, async (req, res) => {
  // Only your own; shared host providers are not yours to remove.
  const removed = pool.remove(fingerprint(getKey(req)), req.params.name);
  if (removed)
    audit({ action: 'provider.remove', actorKey: getKey(req), targetType: 'provider', targetId: req.params.name, req });
  if (removed) await keyConfig.saveRouting(getKey(req), pool);
  res.status(removed ? Status.OK : Status.NOT_FOUND).json(removed ? { ok: true } : { error: 'Provider not found.' });
});

/** POST /gateway/strategy — sets how this key's sessions are routed across its providers. */
router.post('/gateway/strategy', authMiddleware, async (req, res) => {
  const strategy = String(req.body?.strategy || '');
  if (!STRATEGIES.includes(strategy)) {
    return res.status(Status.BAD_REQUEST).json({ error: `strategy must be one of ${STRATEGIES.join(', ')}` });
  }
  await applyStrategy(req, strategy);
  res.json({ ok: true, strategy });
});

/** Per key: how you want your sessions routed is not a host-wide switch. Saved and audited. */
async function applyStrategy(req, strategy) {
  const owner = fingerprint(getKey(req));
  const previous = pool.strategyFor(owner);
  pool.setStrategy(owner, strategy);
  await keyConfig.saveRouting(getKey(req), pool);
  const meta = { from: previous, to: strategy };
  audit({ action: 'routing.strategy', actorKey: getKey(req), targetType: 'routing', meta, req });
}

// Profiles and recordings are per-key. Names and session ids are caller-chosen
// or guessable, so they are scoped by owner rather than treated as secrets.
/** GET /gateway/profiles — this key's saved browser profiles. */
router.get('/gateway/profiles', authMiddleware, async (req, res) =>
  res.json({ profiles: await profiles.list(fingerprint(getKey(req))) }),
);

/** DELETE /gateway/profiles/:name — deletes one of this key's profiles. */
router.delete('/gateway/profiles/:name', authMiddleware, async (req, res) => {
  const removed = await profiles.remove(fingerprint(getKey(req)), req.params.name);
  auditProfileDelete(req, removed);
  res.json({ ok: removed });
});

/** Audits a profile deletion, noting whether there was anything to delete. */
function auditProfileDelete(req, removed) {
  audit({
    action: 'profile.delete',
    actorKey: getKey(req),
    targetType: 'profile',
    targetId: req.params.name,
    outcome: removed ? 'ok' : 'error',
    req,
  });
}

/** GET /gateway/recordings — this key's session recordings. */
router.get('/gateway/recordings', authMiddleware, async (req, res) =>
  res.json({ recordings: await recorder.list(fingerprint(getKey(req))) }),
);

/** GET /gateway/recordings/:id — one recording's manifest. */
router.get('/gateway/recordings/:id', authMiddleware, async (req, res) => {
  const found = await recorder.manifest(req.params.id, ownerScope(req));
  if (!found) return res.status(Status.NOT_FOUND).json({ error: 'No such recording' });
  res.json(found);
});

/** One frame of a recording, for the dashboard player to scrub through. */
router.get('/gateway/recordings/:id/frames/:index', authMiddleware, async (req, res) => {
  const buf = await recorder.frame(req.params.id, req.params.index, ownerScope(req));
  if (!buf) return res.status(Status.NOT_FOUND).json({ error: 'No such frame' });
  res.type('image/jpeg').set('Cache-Control', 'private, max-age=3600').send(buf);
});

/** DELETE /gateway/recordings/:id — deletes one of this key's recordings. */
router.delete('/gateway/recordings/:id', authMiddleware, async (req, res) => {
  const removed = await recorder.remove(req.params.id, ownerScope(req));
  audit({ action: 'recording.delete', actorKey: getKey(req), targetType: 'recording', targetId: req.params.id, req });
  res.json({ ok: removed });
});
