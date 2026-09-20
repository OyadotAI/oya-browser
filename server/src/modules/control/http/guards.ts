/** Request helpers the control routes share: the caller's key, the administrator guard, browser ownership. */
import { fault } from '../service.ts';
import { registry } from '../../browsers/registry.ts';
import { Status } from '../../../platform/http-status.ts';

/** The project key the caller authenticated as. */
export const key = (req) => req.principal.key;

/** Route guard: administrators only. */
export const admin = (req, res, next) =>
  req.principal.role === 'administrator'
    ? next()
    : res.status(Status.FORBIDDEN).json({ error: 'Administrator permission required' });

/** Throws 404 unless the route's browser is connected here and belongs to the caller. */
export function requireOwnBrowser(req) {
  const browser = registry.get(req.params.id);
  if (!browser || browser.apiKey !== key(req)) throw fault('not_found', 'Browser not connected', Status.NOT_FOUND);
}

/** Hands the request to POST /browsers/start; `failure` is the error answered if that handler fails. */
export async function redispatchStart(req, res, failure) {
  const { router } = await import('../../../app/api.ts');
  req.url = '/browsers/start';
  (router as any).handle(req, res, (error) => {
    if (error) res.status(Status.UNAVAILABLE).json({ error: failure });
  }); // untyped in @types/express 5
}
