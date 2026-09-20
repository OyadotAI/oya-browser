/**
 * REST routes: browsers. Each route validates, calls the handler that does the
 * work (http/) and answers; starting and stopping live in lifecycle/.
 */

import { Router } from 'express';
import { authMiddleware } from '../auth/service.ts';
import { registry } from './registry.ts';
import { isConfigured as sandboxConfigured, listSandboxBrowsers } from '../../drivers/sandbox.ts';
import { enforce } from '../../platform/limits.ts';
import { available as availableProviders } from '../../drivers/providers.ts';
import * as keyConfig from '../config/service.ts';
import { admission } from '../control/admission.ts';
import { getKey, requireBrowser } from '../../app/http.ts';
import { startBrowser } from './lifecycle/start.ts';
import { stopOne, stopMany, disconnect, disconnectAll, detach } from './http/stop.ts';
import { browserDetail, liveView } from './http/inspect.ts';
import { connectCdp } from './http/connect.ts';
import { solveCaptcha, completeMfa } from './http/challenges.ts';
import { provision, deleteSandbox } from './http/provision.ts';
import { runCommand, chat } from './http/command.ts';

export { startBrowser } from './lifecycle/start.ts';
export { stopBrowser } from './lifecycle/stop.ts';

/** Browser routes, mounted on the API router. */
export const router = Router({ caseSensitive: true });

// ─── Control ─────────────────────────────────────────────────────────────────

/** POST /browsers/:browserId/stop — stops one browser (see stopBrowser). */
router.post('/browsers/:browserId/stop', authMiddleware, stopOne);

/** Bulk stop: `{ids: [...]}` or `{all: true}`. Each id reports separately. */
router.post('/browsers/stop', authMiddleware, stopMany);

/** One browser with its recent activity — what the detail panel polls. */
router.get('/browsers/:browserId', authMiddleware, browserDetail);

/** Force a browser off the fleet — a stuck client, a runaway, an abusive key. */
router.post('/browsers/:browserId/disconnect', authMiddleware, disconnect);

/** Drop every browser on the calling key. */
router.post('/browsers/disconnect-all', authMiddleware, disconnectAll);

// ─── Browser providers (CDP) ─────────────────────────────────────────────────

/** GET /providers — CDP providers configured for this key, and whether Oya Cloud is available. */
router.get('/providers', authMiddleware, (req, res) => {
  res.json({ providers: availableProviders(keyConfig.envFor(getKey(req))), oyaCloud: sandboxConfigured() });
});

/**
 * Attach a CDP browser: one we dial out to, rather than one that dials in.
 * Anchor, Browserbase, Steel, or any Chrome with --remote-debugging-port.
 */
router.post('/browsers/connect', authMiddleware, enforce('connect'), admission('cdp'), connectCdp);

/** Detach a CDP browser and release the vendor session. */
router.delete('/browsers/:browserId/connection', authMiddleware, detach);

/** POST /browsers/start — starts a browser on whichever provider is configured. */
router.post('/browsers/start', authMiddleware, enforce('provision'), admission(), startBrowser);

/** POST /browsers/:browserId/captcha — detects a CAPTCHA on the page and, unless `solve` is false, solves it. */
router.post('/browsers/:browserId/captcha', authMiddleware, enforce('command'), requireBrowser, solveCaptcha);

/** POST /browsers/:browserId/mfa — completes an MFA prompt on the page for the browser's persona. */
router.post('/browsers/:browserId/mfa', authMiddleware, enforce('command'), requireBrowser, completeMfa);

// ─── Oya Cloud browser provisioning ───
//
// Launches sandboxed browsers that enroll over the normal WebSocket with the
// caller's own key, so they join that caller's pool as ordinary browsers.

/** POST /browsers/provision — launches up to 100 Oya Cloud sandbox browsers, within the hourly quota. */
router.post('/browsers/provision', authMiddleware, enforce('provision'), provision);

/** DELETE /browsers/:browserId/sandbox — destroys a browser's Oya Cloud sandbox; ownership is checked by its owner label. */
router.delete('/browsers/:browserId/sandbox', authMiddleware, deleteSandbox);

/** GET /browsers — connected browsers, scoped to the caller's API key (admin sees all). */
router.get('/browsers', authMiddleware, async (req, res) => {
  const key = getKey(req);
  res.json(await listSandboxBrowsers(key, registry.list(key)));
});

/**
 * GET /live/:browserId — live view, an SSE stream of JPEG frames.
 *
 * Header auth, or ?ticket= for EventSource, which cannot set headers. A ticket
 * is single-use and lives 60 seconds (control().ticket / redeem, redeemed in
 * forwardHttp). ?key= used to be accepted here: that put a project's permanent
 * administrator credential into browser history, Referer headers, proxy and
 * CDN access logs, and — via `oya open` — the process argv table.
 */
router.get('/live/:browserId', authMiddleware, requireBrowser, liveView);

/** POST /browsers/:browserId/command — runs one action on a browser. Server-internal actions are refused. */
router.post('/browsers/:browserId/command', authMiddleware, enforce('command'), requireBrowser, runCommand);

/** POST /browsers/:browserId/chat — LLM + MCP tools for natural-language browser control. */
router.post('/browsers/:browserId/chat', authMiddleware, enforce('chat'), requireBrowser, chat);
