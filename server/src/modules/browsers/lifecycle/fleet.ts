/**
 * Small operations on the fleet that starting, stopping and the routes share:
 * counting, dropping and forgetting browsers, the browser quota, display names.
 */
import { registry } from '../registry.ts';
import { invalid, sendError } from '../../../platform/errors.ts';
import { metrics } from '../../../platform/metrics.ts';
import { checkQuota } from '../../../platform/limits.ts';
import { MAX_NAME, OPERATOR_CLOSE } from '../constants.ts';
import { releaseBrowser } from '../connection/release.ts';

/** Publishes how many browsers this replica holds. */
export function countBrowsers() {
  metrics.browsersConnected.set({}, registry.browsers.size);
}

/**
 * Gives back everything the browser held and removes it from the registry, then closes its
 * socket, if it has one, with `reason`. Released here, not left to the socket: its close
 * comes later, when the browser is no longer its to release.
 */
export function dropBrowser(browserId, ws, reason: string) {
  const browser = registry.get(browserId);
  if (browser) releaseBrowser(browserId, browser, 'disconnected');
  try {
    ws?.close(OPERATOR_CLOSE, reason);
  } catch {}
}

/** A driver closed on its own: forget the browser if it is still registered. */
export function forget(browserId) {
  if (registry.get(browserId)) registry.remove(browserId);
}

/** The browser quota for `key`, counting the browsers it already holds here. */
export function browserQuota(key) {
  const mine = [...registry.browsers.values()].filter((b) => b.apiKey === key).length;
  return checkQuota('browsers', key, mine);
}

/** The 429 body for a reached browser quota. */
export const quotaBody = (quota) => ({
  error: `Browser quota reached (${quota.quota})`,
  code: 'quota_exceeded',
  ...quota,
});

/**
 * Answers 400 for a browser name that is not a string, truthy when it answered.
 * Both start routes ask before anything is acquired, so a typo costs nothing:
 * found later, it failed a start that already held a session and a socket.
 */
export function refuseBadName(res, name) {
  const bad = name !== undefined && name !== null && typeof name !== 'string';
  if (bad) sendError(res, invalid('name', 'a string', name));
  return bad;
}

/** The name a CDP browser is listed under: the caller's, else the vendor's. */
export const displayName = (req, session) => (req.body?.name || `${session.provider} browser`).slice(0, MAX_NAME);
