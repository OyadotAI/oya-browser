/**
 * Stopping one browser, whatever it is. This is what the dashboard's Stop does.
 *
 *   oya-cloud   destroy the Oya Cloud sandbox (or it redials and keeps billing),
 *               then drop the socket and the registry entry
 *   cdp         registry.remove(), which closes the driver and releases the
 *               vendor session
 *   desktop     close the socket
 */
import { registry } from '../registry.ts';
import { canAccess, getKey } from '../../../app/http.ts';
import { mergeDump, drain as drainLogins } from '../../personas/cookies.ts';
import { audit } from '../../../platform/audit.ts';
import { stopAbsent } from './stop-absent.ts';
import { stopConnected } from './stop-connected.ts';
import { notConnected, type StopResult } from './stop-results.ts';

/**
 * Stops one browser. Returns what actually happened, so a Stop that could not
 * reach Oya Cloud is visible rather than reported as done.
 */
export async function stopBrowser(req, browserId, { sandbox, force = false }: any = {}): Promise<StopResult> {
  const key = getKey(req);
  const browser = registry.get(browserId);
  if (!browser) return stopAbsent(req, key, browserId, force);
  if (!canAccess(req, browserId)) return notConnected(browserId);
  const unsaved = browser.driver && browser.persona ? await saveProfile(key, browserId, browser, force) : null;
  return unsaved || stopConnected(req, key, browserId, browser, sandbox);
}

/** Pulls the browser's cookies into its persona before it goes. */
async function captureProfile(browser) {
  const { cookies } = await browser.driver.conn.send('Network.getAllCookies', {}, browser.driver.sessionId);
  mergeDump(browser.persona.id, cookies);
  await drainLogins();
}

/** Saves the profile; a failure stops the stop unless `force`, which only audits it. */
async function saveProfile(key, browserId, browser, force) {
  try {
    await captureProfile(browser);
  } catch (err) {
    if (!force) return { id: browserId, ok: false, error: `Could not save profile before stopping: ${err.message}` };
    audit({ action: 'profile.capture.failed', actorKey: key, targetId: browserId, outcome: 'error' });
  }
  return null;
}
