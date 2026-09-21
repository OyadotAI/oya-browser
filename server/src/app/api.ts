/**
 * REST API: mounts the resource routers under /api.
 */

import { Router } from 'express';
import { registry } from '../modules/browsers/registry.ts';
import { metrics } from '../platform/metrics.ts';
import { Status } from '../platform/http-status.ts';
import { STATUS_CLASS_SIZE } from './constants.ts';
import * as siteLogin from '../modules/challenges/login.ts';
import { slackRouter } from '../modules/slack/service.ts';
import { forwardHttp } from '../modules/control/cluster.ts';
import { projectAccountRouter } from '../modules/control/membership.ts';
import { controlRouter } from '../modules/control/routes.ts';
import { router as authRoutes } from '../modules/auth/routes.ts';
import { router as fleetRoutes } from '../modules/fleet/routes.ts';
import { router as browsersRoutes } from '../modules/browsers/routes.ts';
import { personaRoutes } from '../modules/personas/routes.ts';
import { container } from './container.ts';
import { router as pairingRoutes } from '../modules/pairing/routes.ts';
import { router as proxiesRoutes } from '../modules/proxies/routes.ts';
import { router as gatewayRoutes } from '../modules/gateway/routes.ts';
import { router as configRoutes } from '../modules/config/routes.ts';
import { router as runsRoutes } from '../modules/playbooks/routes.ts';
import { router as poolRoutes } from '../modules/browsers/pool-routes.ts';

// Imported by control/routes.js, control/worker.js and the tests.
export { MAX_FILE_BYTES, validData } from './http.ts';
export { startBrowser, stopBrowser } from '../modules/browsers/routes.ts';

/**
 * caseSensitive: Express matches routes case-insensitively by default, but the
 * role guards in authMiddleware test req.path, which keeps the client's
 * casing. `GET /api/Pool/Cookies` would otherwise route to the handler while
 * slipping past the guard that names `/pool/cookies`.
 */
export const router = Router({ caseSensitive: true });

/** Requests for a browser another replica owns are forwarded to that replica. */
router.use(forwardHttp);
/** /control, control-plane sessions, members, credentials, webhooks and audit. */
router.use('/control', controlRouter);
/** /slack, Slack alert installation and settings. */
router.use('/slack', slackRouter);
/** /auth/projects, the signed-in account's projects: list, join, rename, delete, keys. */
router.use('/auth/projects', projectAccountRouter);

/**
 * HTTP metrics. Labelled by the route pattern, never the concrete path, at
 * this fleet size /browsers/:id/command must not become one series per browser.
 */
router.use((req, res, next) => {
  const started = Date.now();
  res.on('finish', () => recordRequest(req, res, started));
  next();
});

/** Count a finished request and its duration under its route label and status class. */
function recordRequest(req, res, started) {
  const route = routeLabel(req);
  metrics.httpRequests.inc({ route, status: `${Math.floor(res.statusCode / STATUS_CLASS_SIZE)}xx` });
  metrics.httpDuration.observe({ route }, Date.now() - started);
}

/** The route pattern, or the path with ids and tokens folded to placeholders. */
function routeLabel(req) {
  return (req.route?.path || req.path)
    .replace(/\/[0-9a-f]{8}-[0-9a-f-]{27,}/gi, '/:id')
    .replace(/\/[A-Za-z0-9_-]{24,}/g, '/:token');
}

// One pair of listeners for the whole process. Registering per browser would
// leak a listener each time and trip EventEmitter's max at 11 CDP browsers.
registry.on('stream:start', ({ id }) => {
  const browser = registry.get(id);
  if (!browser?.driver?.startScreencast) return; // Oya clients push frames themselves
  browser.driver
    .startScreencast((dataUrl) => {
      registry.pushFrame(id, dataUrl);
      metrics.frames.inc({ client: 'cdp' });
    })
    .catch(() => {});
});

registry.on('stream:stop', ({ id }) => {
  registry
    .get(id)
    ?.driver?.stopScreencast?.()
    .catch(() => {});
});

// Sign-in attempts are counted per browser so a wrong password cannot be typed
// until the account locks. A browser that is gone cannot lock anything, and
// keeping its tally would refuse a later browser that reused the id.
registry.on('browser:disconnected', ({ id }) => siteLogin.forget(id));

/** Resources. Mounted without a prefix so req.path, which the role guards read, is unchanged. Auth: sign-in, account and API keys. */
router.use(authRoutes);
/** Health, metrics, usage, audit and fleet operations. */
router.use(fleetRoutes);
/** Browser lifecycle, commands, live view and providers. */
router.use(browsersRoutes);
/** Personas and their credentials, MFA and proxy. */
router.use(personaRoutes(container.personas));
/** Desktop app pairing codes. */
router.use(pairingRoutes);
/** Saved proxies. */
router.use(proxiesRoutes);
/** CDP gateway providers, profiles, sessions and recordings. */
router.use(gatewayRoutes);
/** Settings for the calling key, and the server-wide runtime config. */
router.use(configRoutes);
/** Playbooks and runs. */
router.use(runsRoutes);
/** The shared browser pool and its cookies. */
router.use(poolRoutes);

/** Errors become JSON; only errors carrying a status expose their message. */
router.use((err, req, res, next) => {
  if (res.headersSent) return next(err);
  res.status(err.status || Status.UNAVAILABLE).json({
    error: err.status ? err.message : 'Operation could not be completed',
    code: err.code || 'operation_failed',
  });
});
