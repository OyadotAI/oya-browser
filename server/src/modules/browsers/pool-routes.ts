/**
 * REST routes: pool.
 */

import { Router } from 'express';
import { authMiddleware } from '../auth/service.ts';
import { poolStats } from './pool.ts';
import { getKey } from '../../app/http.ts';
import { poolCommand, clearJar, exportJar, importJar } from './http/pool.ts';

/** Pool routes: status, round-robin commands and persona cookie jars. */
export const router = Router({ caseSensitive: true });

// ─── Pool Endpoints ─────────────────────────────────────────────────────────

/** GET /pool, pool status for the caller's key: how many browsers, who's connected. */
router.get('/pool', authMiddleware, (req, res) => {
  const key = getKey(req);
  res.json(poolStats(key));
});

/** POST /pool/command, send a command to the next browser in the caller's pool, round-robin. */
router.post('/pool/command', authMiddleware, poolCommand);

/**
 * GET /pool/cookies, the cookie jar of the caller's persona (?persona=, else the default),
 * in ?format=json (default), playwright or netscape.
 * Cookies live with the persona, not the key, the jar and the fingerprint
 * have to move together or a returning session looks like a new device.
 */
router.get('/pool/cookies', authMiddleware, exportJar);

/**
 * PUT /pool/cookies, merge `{ cookies: [...] }` into one of the caller's persona jars
 * (?persona=, else the default); audited. How logins exported from another browser,
 * or from another persona, are brought in.
 */
router.put('/pool/cookies', authMiddleware, importJar);

/** DELETE /pool/cookies, clear one of the caller's persona jars (?persona=, else the default); audited. */
router.delete('/pool/cookies', authMiddleware, clearJar);
