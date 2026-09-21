/**
 * The start that needs no configuration. With no provider chosen and no CDP
 * URL to dial, a start can only fail, so the browser already connected on this
 * key is the one the caller meant: the desktop app is usually the first and
 * only browser a new key has. A configured deployment never reaches this file.
 */
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
