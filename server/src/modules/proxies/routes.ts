/**
 * REST routes: proxies.
 */

import { Router } from 'express';
import { authMiddleware } from '../auth/service.ts';
import { audit, fingerprint } from '../../platform/audit.ts';
import * as proxies from './service.ts';
import { getKey } from '../../app/http.ts';
import { Status } from '../../platform/http-status.ts';

/** Proxy routes, mounted by the API. */
export const router = Router({ caseSensitive: true });

// ─── Proxies ─────────────────────────────────────────────────────────────────
//
// A proxy is part of a persona's identity, assigned once and kept, an exit IP
// that changes mid-life looks like an account takeover.

/** GET /proxies, the caller's registered proxies. */
router.get('/proxies', authMiddleware, (req, res) => {
  res.json({ proxies: proxies.list(fingerprint(getKey(req))) });
});

/** POST /proxies, register a proxy (label, url, geo, kind, maxPersonas). */
router.post('/proxies', authMiddleware, async (req, res) => {
  const created = await proxies.register(registrationFrom(req));
  auditCreated(req, created);
  res.status(Status.CREATED).json(created.toJSON());
});

/** The proxy the request describes, owned by the calling key. */
function registrationFrom(req) {
  return {
    owner: fingerprint(getKey(req)),
    label: req.body?.label,
    url: req.body?.url,
    geo: req.body?.geo,
    kind: req.body?.kind,
    maxPersonas: req.body?.maxPersonas,
  };
}

/** Audits a registration; the URL and credentials stay out of the log. */
function auditCreated(req, created) {
  audit({
    action: 'proxy.create',
    actorKey: getKey(req),
    targetType: 'proxy',
    targetId: created.id,
    meta: { geo: created.geo, kind: created.kind },
    req,
  });
}

/** DELETE /proxies/:id, remove one of the caller's proxies. */
router.delete('/proxies/:id', authMiddleware, (req, res) => {
  const removed = proxies.remove(fingerprint(getKey(req)), req.params.id);
  if (!removed) return res.status(Status.NOT_FOUND).json({ error: 'No such proxy' });
  audit({ action: 'proxy.delete', actorKey: getKey(req), targetType: 'proxy', targetId: req.params.id, req });
  res.json({ ok: true });
});

/** Verify each proxy works and report the address its traffic actually leaves from. */
router.post('/proxies/check', authMiddleware, async (req, res) => {
  const results = await proxies.checkAll(fingerprint(getKey(req)));
  res.json({ results });
});
