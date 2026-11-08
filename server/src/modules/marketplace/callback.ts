/** AWS's cross-site form POST is exchanged before Next handles the clean GET page. */
import { Router, urlencoded, type Request, type Response } from 'express';
import { Status } from '../../platform/http-status.ts';
import type { HttpError } from '../../platform/errors.ts';
import { consume } from '../../platform/limits.ts';
import { clientAddress } from '../auth/service.ts';
import { invalidSetup } from './model.ts';
import { unavailable, type MarketplaceConfig } from './config.ts';
import { clearSetupCookies, privateSetupResponse, writeSetupCookies } from './cookies.ts';
import {
  FULFILLMENT_PATH,
  TOKEN_FIELD,
  OFFER_FIELD,
  FORM_LIMIT_BYTES,
  FORM_PARAMETER_LIMIT,
  MAX_TOKEN,
} from './constants.ts';
import type { MarketplaceService } from './service.ts';

/** Rejects duplicate/unknown fields before passing opaque data to AWS. */
function tokenOf(body: unknown): string {
  if (!body || typeof body !== 'object' || Array.isArray(body)) throw invalidSetup();
  const fields = body as Record<string, unknown>;
  const token = fields[TOKEN_FIELD];
  if (Object.keys(fields).some((key) => ![TOKEN_FIELD, OFFER_FIELD].includes(key))) throw invalidSetup();
  if (typeof token !== 'string' || !token.length || token.length > MAX_TOKEN) throw invalidSetup();
  if (fields[OFFER_FIELD] !== undefined && typeof fields[OFFER_FIELD] !== 'string') throw invalidSetup();
  return token;
}

/** Starts fresh before parsing, so even parser errors remove any previous purchase. */
function startCallback(req: Request, res: Response, next: () => void) {
  privateSetupResponse(res);
  clearSetupCookies(res);
  // The caller's own address, not req.ip: behind the ingress that is the proxy, and one shared
  // bucket let anyone posting junk here turn every real buyer away.
  const limit = consume('connect', `aws:${clientAddress(req)}`);
  if (!limit.allowed) {
    res.redirect(Status.SEE_OTHER, `${FULFILLMENT_PATH}?setup=unavailable`);
    return;
  }
  next();
}

/** Only safe enumerated state names enter the redirect URL. */
function callbackFailure(error: unknown, res: Response) {
  const code = (error as HttpError)?.code;
  const state = code === 'unavailable' ? 'unavailable' : 'invalid';
  clearSetupCookies(res);
  return res.redirect(Status.SEE_OTHER, `${FULFILLMENT_PATH}?setup=${state}`);
}

/** Resolves the handoff without putting its token in the browser URL or cookies. */
async function receive(req: Request, res: Response, service: MarketplaceService, config: () => MarketplaceConfig) {
  try {
    const value = await service.begin(tokenOf(req.body));
    writeSetupCookies(res, value, config().origin);
    res.redirect(Status.SEE_OTHER, FULFILLMENT_PATH);
  } catch (error) {
    callbackFailure(error, res);
  }
}

/** Mount only at the fulfillment path, before the frontend and its body handling. */
export function marketplaceCallback(service: MarketplaceService, config: () => MarketplaceConfig) {
  const router = Router({ caseSensitive: true });
  const parser = urlencoded({ extended: false, limit: FORM_LIMIT_BYTES, parameterLimit: FORM_PARAMETER_LIMIT });
  /** Only AWS's documented form-encoded handoff is accepted. */
  router.post('/', startCallback, parser, (req, res) => receive(req, res, service, config));
  router.use((err, _req, res, _next) => callbackFailure(err?.status ? err : unavailable(), res));
  return router;
}
