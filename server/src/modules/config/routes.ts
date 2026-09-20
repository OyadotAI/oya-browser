/**
 * REST routes: config.
 */

import { Router } from 'express';
import { authMiddleware } from '../auth/service.ts';
import { runtimeConfig } from '../../platform/runtime-config.ts';
import { audit } from '../../platform/audit.ts';
import * as keyConfig from './service.ts';
import { getKey, operatorOnly } from '../../app/http.ts';

/** Config routes, mounted on the API router. */
export const router = Router({ caseSensitive: true });

/** Records a config change in the audit trail. */
const auditUpdate = (req, actorKey, meta) =>
  audit({ action: 'config.update', actorKey, targetType: 'config', meta, req });

/**
 * GET /config — the calling key's settings (OpenAI key, model, base URL).
 *
 * Settings belong to the API key that presents them — the same identity that
 * owns the browsers, personas and cookies. Nothing is inherited from an
 * account, and nothing has to be in the environment; a key that has set
 * nothing falls back to the host defaults.
 */
router.get('/config', authMiddleware, (req, res) => {
  res.json(keyConfig.get(getKey(req)));
});

/**
 * POST /config — updates the calling key's settings. Administrator credentials
 * only (authMiddleware): whoever can write this controls where that key's chat
 * requests go.
 */
router.post('/config', authMiddleware, async (req, res) => {
  const key = getKey(req);
  await keyConfig.set(key, req.body);
  auditUpdate(req, key, { fields: Object.keys(req.body || {}) });
  res.json({ ok: true, ...keyConfig.get(key) });
});

/**
 * POST /config/host — sets the deployment-wide default. It affects every key
 * that has not set its own, so it stays behind the operator token.
 */
router.post('/config/host', operatorOnly, async (req, res) => {
  await runtimeConfig.set(req.body);
  auditUpdate(req, getKey(req), { scope: 'host', fields: Object.keys(req.body || {}) });
  res.json({ ok: true, scope: 'host', ...runtimeConfig.get() });
});
