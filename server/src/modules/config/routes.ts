/**
 * REST routes: config.
 */

import { Router } from 'express';
import { Status } from '../../platform/http-status.ts';
import { authMiddleware } from '../auth/service.ts';
import { runtimeConfig } from '../../platform/runtime-config.ts';
import { audit } from '../../platform/audit.ts';
import * as keyConfig from './service.ts';
import * as chatgpt from './chatgpt.ts';
import { getKey, operatorOnly } from '../../app/http.ts';
import { registry } from '../browsers/registry.ts';
import { LLM_FIELDS } from './fields.ts';
import { enforce } from '../../platform/limits.ts';
import { HttpError } from '../../platform/errors.ts';
import { MODEL_TEST_PROVIDERS } from './constants.ts';
import { testModel } from './model-test.ts';

/** Config routes, mounted on the API router. */
export const router = Router({ caseSensitive: true });

/** Records a config change in the audit trail. */
const auditUpdate = (req, actorKey, meta) =>
  audit({ action: 'config.update', actorKey, targetType: 'config', meta, req });

/**
 * GET /config, the calling key's settings (OpenAI key, model, base URL).
 *
 * Settings belong to the API key that presents them, the same identity that
 * owns the browsers, personas and cookies. Nothing is inherited from an
 * account, and nothing has to be in the environment; a key that has set
 * nothing falls back to the host defaults.
 */
router.get('/config', authMiddleware, (req, res) => {
  res.json(keyConfig.get(getKey(req)));
});

/** POST /config/model-test sends one fixed request using only the caller's saved provider key. */
router.post('/config/model-test', authMiddleware, enforce('chat'), async (req, res) => {
  res.json(await modelTestRequest(req, res));
});
/** Reject unknown diagnostic fields instead of turning the endpoint into a general model proxy. */
function modelTestProvider(body): string {
  if (!body || Object.keys(body).join(',') !== 'provider' || !MODEL_TEST_PROVIDERS.has(body.provider))
    throw new HttpError(Status.BAD_REQUEST, 'Choose a supported provider.', { code: 'invalid_request' });
  return body.provider;
}
/** Caller disconnect aborts the underlying provider request, not just its response wait. */
async function modelTestRequest(req, res) {
  const abort = new AbortController();
  const closed = modelTestClose(res, abort);
  try {
    return await testModel(getKey(req), modelTestProvider(req.body), abort.signal);
  } finally {
    res.removeListener('close', closed);
  }
}

/** A completed HTTP response does not cancel an already finished diagnostic. */
function modelTestClose(res, abort: AbortController): () => void {
  const closed = () => {
    if (!res.writableEnded) abort.abort();
  };
  res.once('close', closed);
  return closed;
}

/**
 * POST /config, updates the calling key's settings. Administrator credentials
 * only (authMiddleware): whoever can write this controls where that key's chat
 * requests go.
 */
router.post('/config', authMiddleware, async (req, res) => {
  const key = getKey(req);
  await keyConfig.set(key, req.body);
  const fields = Object.keys(req.body || {});
  auditUpdate(req, key, { fields });
  // The desktop's model card shows what the server runs on: tell open ones to re-read it.
  // ponytail: this replica's desktops only; the rest re-read when their card next opens. A cluster bus if that lags.
  if (fields.some((f) => LLM_FIELDS.includes(f))) registry.tell(key, { type: 'settings_changed', scope: 'llm' });
  res.json({ ok: true, ...keyConfig.get(key) });
});

/**
 * POST /config/chatgpt/login, starts Sign in with ChatGPT for the calling key and
 * answers `{ url }` to open in a browser on the server's own machine (the sign-in
 * returns to a loopback address). Self-hosted only; settings writes need an administrator.
 */
router.post('/config/chatgpt/login', authMiddleware, async (req, res) => {
  res.json({ url: await chatgpt.startLogin(getKey(req)) });
});

/**
 * GET /config/site-notes, what the agent chose to remember about each site for
 * this key. The agent writes these from what it read on a page, so the owner
 * needs to be able to see them, and a hostile page's suggestion is removable.
 */
router.get('/config/site-notes', authMiddleware, (req, res) => {
  res.json({ notes: keyConfig.allSiteNotes(getKey(req)) });
});

/** DELETE /config/site-notes/:host, forgets what the agent kept about one site. */
router.delete('/config/site-notes/:host', authMiddleware, async (req, res) => {
  const key = getKey(req);
  await keyConfig.forgetSiteNotes(key, req.params.host);
  auditUpdate(req, key, { scope: 'site-notes', host: req.params.host });
  res.json({ ok: true, notes: keyConfig.allSiteNotes(key) });
});

/**
 * POST /config/host, sets the deployment-wide default. It affects every key
 * that has not set its own, so it stays behind the operator token.
 */
router.post('/config/host', operatorOnly, async (req, res) => {
  await runtimeConfig.set(req.body);
  auditUpdate(req, getKey(req), { scope: 'host', fields: Object.keys(req.body || {}) });
  res.json({ ok: true, scope: 'host', ...runtimeConfig.get() });
});

/** A small page for the browser tab sign-in returns to, so the person knows to go back to Oya. */
const callbackPage = (title: string, line: string) =>
  `<!doctype html><meta charset="utf-8"><title>${title}</title>` +
  `<body style="font:16px system-ui;max-width:32rem;margin:15vh auto;padding:0 1rem"><h1>${title}</h1><p>${line}</p>`;

/** Escapes text for the callback page; the account email comes from OpenAI, but it is still not ours. */
const escapeHtml = (text: string) => text.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);

/** Whether a return to /auth/callback is ChatGPT's (its state, with a client, scope or refusal), not the console's. */
const isChatgptReturn = (query) => typeof query.state === 'string' && !!(query.client_id || query.scope || query.error);

/** Signs the key in from the callback and tells its desktops; the line the page shows. */
async function connected(query) {
  const { apiKey, email } = await chatgpt.finishLogin(query || {});
  registry.tell(apiKey, { type: 'settings_changed', scope: 'llm' });
  return `Oya now runs on your ChatGPT plan${email ? ` as ${escapeHtml(email)}` : ''}. You can close this tab.`;
}

/**
 * GET /auth/callback (mounted at the root, outside /api's key check): where
 * Sign in with ChatGPT returns. The single-use state names the key.
 */
export async function chatgptCallback(req, res, next) {
  if (!isChatgptReturn(req.query || {})) return next(); // the console's own OAuth callback page
  const answered = await connected(req.query).then(
    (line) => ({ status: Status.OK, page: callbackPage('ChatGPT connected', line) }),
    (err) => notConnected(err),
  );
  res.status(answered.status).type('html').send(answered.page);
}

/** A failed sign-in's page; the key's desktops, whose card is waiting on it, are told to re-read. */
function notConnected(err) {
  if (err.apiKey) registry.tell(err.apiKey, { type: 'settings_changed', scope: 'llm' });
  return {
    status: err.status || Status.INTERNAL,
    page: callbackPage('ChatGPT not connected', escapeHtml(err.message)),
  };
}
