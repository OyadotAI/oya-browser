/**
 * REST routes: pairing.
 */

import { Router } from 'express';
import { authMiddleware } from '../auth/service.ts';
import { audit, fingerprint } from '../../platform/audit.ts';
import { enforce, consume } from '../../platform/limits.ts';
import { container } from '../../app/container.ts';
import * as pairing from './service.ts';
import { getKey } from '../../app/http.ts';
import { Status } from '../../platform/http-status.ts';

// Not yet layered: reads the persona service from the composition root.
const { personas } = container;

/** Desktop pairing routes. */
export const router = Router({ caseSensitive: true });

// ─── Desktop pairing ─────────────────────────────────────────────────────────
//
// The `oya://` link the dashboard builds carries one of these codes, never the
// API key. See pairing.js for why.

/** POST /pairing — a short-lived code the desktop app redeems for the caller's key, bound to a persona. */
router.post('/pairing', authMiddleware, enforce('provision'), (req, res) => {
  const key = getKey(req);
  const persona = personas.resolve(key, req.body?.profile || req.body?.persona);
  const { code, expiresAt } = pairing.issue(key, persona.id);
  audit({ action: 'pairing.issue', actorKey: key, targetType: 'key', targetId: fingerprint(key), req });
  res.status(Status.CREATED).json({ code, expiresAt });
});

/** The address a claim is rate limited by. */
const clientAddress = (req) => req.ip || req.socket?.remoteAddress || 'unknown';

/**
 * Redeem a code for the key it stands for. Unauthenticated by necessity — the
 * desktop app has no credential yet, which is the whole point — so the code is
 * 256 bits, single use, and short-lived, and the attempt is rate limited by IP.
 */
router.post('/pairing/claim', (req, res) => {
  if (!consume('connect', `pair:${clientAddress(req)}`).allowed) {
    return res.status(Status.TOO_MANY_REQUESTS).json({ error: 'Too many pairing attempts' });
  }
  const paired = pairing.claimDetails(req.body?.code);
  if (!paired) return res.status(Status.NOT_FOUND).json({ error: 'That pairing code is invalid, used or expired' });
  const { apiKey, persona } = paired;
  audit({ action: 'pairing.claim', actorKey: apiKey, targetType: 'key', targetId: fingerprint(apiKey), req });
  res.json({ apiKey, persona });
});
