/**
 * Stopping a browser this replica holds: its managed session, its Oya Cloud
 * sandbox, its vendor session and its socket, in that order.
 */
import { isConfigured as sandboxConfigured, removeSandbox } from '../../../drivers/sandbox.ts';
import * as usage from '../../../platform/usage.ts';
import { Status } from '../../../platform/http-status.ts';
import { control } from '../../control/service.ts';
import { countBrowsers, dropBrowser } from './fleet.ts';
import { auditStop } from './stop-results.ts';

/** Stops `browser`; a vendor that will not release it is reported, not hidden. */
export async function stopConnected(req, key, browserId, browser, sandbox) {
  if ((await control().findSession(key, browserId))?.runtime) return cancelManaged(key, browserId);
  const sandboxRemoved = await removeOwnSandbox(key, browserId, browser, sandbox);
  const durable = await control().findSession(key, browserId);
  if (durable) await control().update(key, browserId, { state: 'cleanup_pending' });
  const failed = await releaseAndDrop(browserId, browser);
  if (failed) return failed;
  if (durable && sandboxRemoved !== false) await control().update(key, browserId, { state: 'stopped' });
  return stopped(req, key, browserId, browser, sandboxRemoved);
}

/** A managed runtime session is cancelled; its runtime cleans up. */
async function cancelManaged(key, browserId) {
  await control().cancel(key, browserId);
  return { id: browserId, ok: true, status: 'cleanup_pending' };
}

/**
 * Whether a sandbox exists is decided by asking Oya Cloud, not by what the
 * browser said about itself: an older image sends no provider, and a stop
 * that trusts the claim leaves a sandbox running and billing. removeSandbox
 * looks the sandbox up by this browser's name and this key's owner label,
 * so for a desktop browser it simply finds nothing. Null when not asked.
 */
async function removeOwnSandbox(key, browserId, browser, sandbox) {
  if (!(browser.clientType === 'oya' && (sandboxConfigured(key) || sandbox === true))) return null;
  try {
    return await removeSandbox(browserId, key);
  } catch (err) {
    console.warn(`[stop] sandbox for ${browserId} not removed: ${err.message}`);
    return false;
  }
}

/** Releases the vendor session, then drops the socket and registry entry; the failure when it would not release. */
async function releaseAndDrop(browserId, browser) {
  const failed = await releaseVendor(browserId, browser);
  if (!failed) dropBrowser(browserId, browser.ws, 'Stopped by operator');
  return failed;
}

/** Hands a hosted session back; a 502 result when the vendor refuses. */
async function releaseVendor(browserId, browser) {
  if (!browser.release) return null;
  try {
    await browser.release();
  } catch (err) {
    return { id: browserId, ok: false, status: Status.BAD_GATEWAY, error: err.message };
  }
  browser.release = null;
  return null;
}

/** Books the stop: usage, audit, metrics, and the result. */
function stopped(req, key, browserId, browser, sandboxRemoved) {
  usage.browserDisconnected(key, browserId);
  auditStop(req, key, browserId, { clientType: browser.clientType, provider: browser.provider, sandboxRemoved });
  countBrowsers();
  // If Oya Cloud had a sandbox for it, it was a cloud browser whatever it claimed.
  const provider = sandboxRemoved ? 'oya-cloud' : browser.provider;
  return { id: browserId, ok: true, sandboxRemoved, provider };
}
