/**
 * Oya Cloud sandbox provisioning — launch cloud browsers on demand.
 *
 * Configured with OYA_CLOUD_*. The older DAYTONA_* names are still read so
 * existing deployments and CI keep working, but they are not the documented
 * spelling: the underlying runtime is an implementation detail.
 *
 * A provisioned sandbox enrolls into the normal registry over the normal
 * WebSocket, using the same OYA_SERVER_URL / OYA_API_KEY / OYA_BROWSER_ID
 * contract browser/main.js already reads. Once connected it is an ordinary
 * browser: /browsers, /live/:id, /browsers/:id/command, /pool/* all work on it
 * with no special-casing anywhere.
 *
 * Sandboxes are named oya-browser-<browserId>, so the sandbox for a browser is
 * addressable without storing anything — a restart can still find and delete it.
 */
import { control } from '../modules/control/service.ts';
import { HttpError } from '../platform/errors.ts';
import { Status } from '../platform/http-status.ts';
import { PREFIX, ownerTag, settings, unconfigured } from './sandbox/config.ts';
import { client, isNotFound } from './sandbox/client.ts';
import { reserveBrowser, launch, startBrowser } from './sandbox/launch.ts';
import { forgetInventory } from './sandbox/inventory.ts';
import { SANDBOX_DELETE_TIMEOUT_S } from './constants.ts';

export { isConfigured, missingSettings } from './sandbox/config.ts';
export { listSandboxBrowsers } from './sandbox/inventory.ts';

/**
 * Browser ids this process created sandboxes for. The browser's own claim
 * about its provider is a courtesy, not a source of truth: an older image
 * does not send one, and a client could say anything. Not persisted — after a
 * restart the upstream lookup by name in removeSandbox is the authority.
 */
const provisioned = new Set();
/** Whether this process created a sandbox for the browser. */
export const isProvisioned = (browserId) => provisioned.has(browserId);

/** The cloud settings, refusing when they are incomplete or no key is given. */
function requireSettings(apiKey) {
  const config = settings();
  if (!config) throw unconfigured();
  if (!apiKey) throw new HttpError(Status.BAD_REQUEST, 'An API key is required');
  return config;
}

/**
 * Create one cloud browser. Resolves once the sandbox is starting — the browser
 * enrolls on its own and shows up in the registry within ~90s.
 */
export async function createSandbox({ apiKey, name, persona, browserId }: any = {}) {
  const config = requireSettings(apiKey);
  const daytona = await client();
  browserId ||= await reserveBrowser(apiKey, persona);
  await control().update(apiKey, browserId, { cleanup: { kind: 'sandbox', browserId } });
  const sandbox = await launch(daytona, config, { apiKey, name, persona, browserId });
  await startBrowser(sandbox, config);
  await markProvisioned(apiKey, browserId);
  return { browserId, sandboxId: sandbox.id, sandboxName: PREFIX + browserId };
}

/** The browser is starting: note it here and in the control plane, and refresh the key's inventory. */
async function markProvisioned(apiKey, browserId) {
  forgetInventory(ownerTag(apiKey));
  provisioned.add(browserId);
  await control().update(apiKey, browserId, { provisioningActive: false });
}

/**
 * Delete the sandbox backing a browser. Returns false if there wasn't one.
 *
 * Ownership is checked against the label, not against the live registry: a
 * sandbox outlives its websocket, and a browser id is not a secret, so a
 * disconnected sandbox must still only be deletable by the key that made it.
 */
export async function removeSandbox(browserId, apiKey) {
  if (!apiKey) throw new HttpError(Status.BAD_REQUEST, 'An API key is required');
  const daytona = await client();
  try {
    return await deleteOwned(daytona, browserId, apiKey);
  } catch (err) {
    if (isNotFound(err)) return false;
    throw err;
  }
}

/** Deletes the browser's sandbox if this key owns it; false (as if missing) when it does not. */
async function deleteOwned(daytona, browserId, apiKey) {
  const sandbox = await daytona.get(PREFIX + browserId);
  if (sandbox.labels?.['oya-browser-id'] !== browserId) throw new Error('Sandbox ownership mismatch');
  // Same shape as "no such sandbox" — don't confirm existence to a non-owner.
  if (sandbox.labels?.['oya-owner'] !== ownerTag(apiKey)) return false;
  await sandbox.delete(SANDBOX_DELETE_TIMEOUT_S, true);
  forgetInventory(ownerTag(apiKey));
  provisioned.delete(browserId);
  return true;
}
