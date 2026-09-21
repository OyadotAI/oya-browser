/**
 * CDP gateway.
 *
 * Serves Chrome's discovery endpoint and a WebSocket that speaks raw CDP, so
 * Playwright, Puppeteer, Stagehand, browser-use and any other CDP client
 * connect to this control plane as if it were a browser, no client changes,
 * no vendor SDK. The gateway picks a provider by the configured routing
 * strategy, fails over if one is down, and queues when everything is busy.
 *
 * The wire is forwarded verbatim. Features that need to observe it (recording,
 * profile capture) use a second, independent CDP connection to the same
 * browser rather than injecting frames into the client's session, so a
 * client's own use of Page.screencast or Network is never disturbed.
 *
 * ponytail: every message is forwarded through JS rather than piped at the
 * socket level. Measured cost is a JSON-free buffer copy per frame; if a
 * profile of a saturated gateway ever shows this dominating, add a raw
 * passthrough for sessions with no features enabled.
 *
 * This file is the module's facade: discovery, the session, its teardown and
 * each stage of the upgrade live in their own files.
 */
import { sessions } from './session-store.ts';

export { sessions, wss } from './session-store.ts';
export { handleJsonVersion, handleJsonList } from './discovery.ts';
export { handleUpgrade } from './upgrade.ts';

/** This key's gateway sessions, or every one with `all`. */
export function listSessions(apiKey, { all = false } = {}) {
  return [...sessions.values()].filter((s) => all || s.apiKey === apiKey).map((s) => s.toJSON());
}

/** Destroy a session by id; false if there is none. */
export async function killSession(id, reason = 'closed by operator') {
  const s = sessions.get(id);
  if (!s) return false;
  await s.destroy(reason);
  return true;
}
