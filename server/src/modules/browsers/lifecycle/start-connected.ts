/**
 * The start that needs no configuration. With no provider chosen and no CDP
 * URL to dial, a start can only fail, so the browser already connected on this
 * key is the one the caller meant: the desktop app is usually the first and
 * only browser a new key has. With none connected, a deployment that runs Oya
 * Cloud starts one there; a configured deployment never reaches this file.
 */
import { getKey } from '../../../app/http.ts';
import { isConfigured as sandboxConfigured } from '../../../drivers/sandbox.ts';
import { managedConfigured } from '../../control/managed.ts';
import { CLOUD_PROVIDER } from '../connection/constants.ts';
import { HttpError } from '../../../platform/errors.ts';
import { Status } from '../../../platform/http-status.ts';
import * as keyConfig from '../../config/service.ts';
import { registry } from '../registry.ts';
import { browserCdpUrl } from './cdp-url.ts';
import { started, type Start } from './start-reply.ts';

/** True when the start names no provider, has no CDP URL to dial and would fail for want of one. */
export function nothingToDial(req, key: string, wanted: string) {
  // A caller who names cdp chose it, and is told it needs a URL rather than handed some other browser.
  if (wanted !== 'cdp' || req.body?.provider) return false;
  return !req.body?.wsUrl && !keyConfig.envFor(key).OYA_CDP_WS_URL;
}

/** The healthiest browser already connected on this key. */
function connected(key: string) {
  const rows = registry.list(key).filter((b) => b.health !== 'unresponsive');
  return rows.find((b) => b.health === 'healthy') || rows[0] || null;
}

/**
 * Whether a start that names nothing and has nothing connected runs on Oya Cloud:
 * only where this deployment runs sandboxes for the key and no governed runtime is
 * in play, since a governed browser must never land outside it.
 */
const cloudCanStart = (req, key: string) =>
  sandboxConfigured(key) && !managedConfigured() && !req.body?.governed && !connected(key);

/**
 * Middleware for /browsers/start, ahead of the billing gate and admission: a start
 * that names no provider and has no browser to borrow is named Oya Cloud here, so
 * the gate charges, admission reserves and the launcher starts the same provider.
 * Without it a first `start()` on a new key was a 409 asking for `oya init`.
 */
export function cloudWhenNothingConnected(req, res, next) {
  const key = getKey(req);
  const wanted = String(req.body?.provider || keyConfig.providerFor(key));
  if (nothingToDial(req, key, wanted) && cloudCanStart(req, key)) req.body = { ...req.body, provider: CLOUD_PROVIDER };
  next();
}

/** The three ways to get a browser, for a key that has none. */
const NOTHING_RUNNING =
  'No browser provider is set for this key and no browser is connected. Three ways on: ' +
  'open the Oya desktop browser and this call will use it, ' +
  'run `npx @oya-ai/cli init` to pick a provider, ' +
  'or pass wsUrl for a Chrome you started with --remote-debugging-port.';

/** The reply for a browser that was already there. */
const borrowed = (start: Start, browser, cdpUrl: string) => ({
  id: browser.id,
  provider: browser.provider || browser.clientType || 'desktop',
  persona: browser.persona || start.persona.id,
  status: 'ready',
  // The caller did not start this one, so nothing of theirs should stop it.
  reused: true,
  cdpUrl,
});

/** Answers with the browser this key already has; throws the ways to get one when it has none. */
export async function startWithConnected(start: Start) {
  const browser = connected(start.key);
  if (!browser) throw new HttpError(Status.CONFLICT, NOTHING_RUNNING);
  started(start, borrowed(start, browser, await browserCdpUrl(start.req, browser.id)));
}
