/** Account-authenticated explicit linking never changes a subscription or grants paid access. */
import { Router, type Request, type Response } from 'express';
import { userAuthMiddleware } from '../auth/service.ts';
import { HttpError, sendError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import type { MarketplaceConfig } from './config.ts';
import type { MarketplaceService } from './service.ts';
import { clearSetupCookies, privateSetupResponse, setupCookie } from './cookies.ts';

/** Body mutation is forbidden: account and purchase identity come from authenticated sources. */
function validateConnection(req: Request, origin: string) {
  if (req.headers.origin !== origin || req.impersonatedBy)
    throw new HttpError(Status.FORBIDDEN, 'Sign in to connect your own purchase', { code: 'connection_forbidden' });
  if (!req.body || typeof req.body !== 'object' || Array.isArray(req.body) || Object.keys(req.body).length)
    throw new HttpError(Status.BAD_REQUEST, 'An empty request body is required');
  if (!/^[A-Za-z0-9_-]{16,128}$/.test(String(req.headers['idempotency-key'] || '')))
    throw new HttpError(Status.BAD_REQUEST, 'A valid Idempotency-Key is required');
}

/** Expired/invalid contexts must stop auth redirects; outages preserve retry context. */
async function answer(req: Request, res: Response, work: () => unknown) {
  privateSetupResponse(res);
  try {
    res.json(await work());
  } catch (error) {
    if (['invalid_setup', 'setup_expired'].includes(error?.code)) clearSetupCookies(res);
    sendError(res, error, req);
  }
}

/** Separate public context from owner-specific status so project keys cannot attach purchases. */
export function marketplaceRoutes(service: MarketplaceService, config: () => MarketplaceConfig) {
  const router = privateRouter();
  /** Anonymous context contains only the buyer account suffix, never its owner. */
  router.get('/marketplace/aws', (req, res) => answer(req, res, () => service.context(setupCookie(req))));
  /** Reads durable registration only after account authentication and MFA. */
  router.get('/marketplace/aws/status', userAuthMiddleware, (req, res) =>
    answer(req, res, () => service.status(setupCookie(req), req.user.id)),
  );
  connectRoute(router, service, config);
  return router;
}

/** Authentication refusals carry the same privacy headers as successful setup responses. */
function privateRouter() {
  const router = Router({ caseSensitive: true });
  /** Apply privacy only to this module's routes, including their authentication failures. */
  router.use('/marketplace/aws', (_req, res, next) => {
    privateSetupResponse(res);
    next();
  });
  return router;
}

/** The explicit click is the only path that persists account ownership. */
function connectRoute(router: Router, service: MarketplaceService, config: () => MarketplaceConfig) {
  /** No GET, authentication callback, or client identity field can connect a purchase. */
  router.post('/marketplace/aws/connect', userAuthMiddleware, (req, res) =>
    answer(req, res, () => {
      validateConnection(req, config().origin);
      return service.connect(setupCookie(req), req.user.id, String(req.headers['x-oya-marketplace-context'] || ''));
    }),
  );
}
