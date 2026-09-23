/**
 * Oya Cloud sandbox provisioning, launch cloud browsers on demand.
 *
 * The facade over every sandbox runtime: Daytona, Docker, Kubernetes and ECS
 * sit behind worker.ts, and callers never learn which one ran a browser.
 * The deployment picks one with OYA_CLOUD_RUNTIME (default daytona); a key can
 * pick its own with the sandbox_runtime setting (see tenancy.ts for which
 * credentials are whose). The older DAYTONA_* names are still read so existing
 * deployments and CI keep working.
 *
 * A provisioned sandbox enrolls into the normal registry over the normal
 * WebSocket, using the same OYA_SERVER_URL / OYA_API_KEY / OYA_BROWSER_ID
 * contract browser/main.js already reads. Once connected it is an ordinary
 * browser: /browsers, /live/:id, /browsers/:id/command, /pool/* all work on it
 * with no special-casing anywhere.
 *
 * Sandboxes are named oya-browser-<browserId>, so the sandbox for a browser is
 * addressable without storing anything, a restart can still find and delete it.
 */
import { control } from '../modules/control/service.ts';
import { HttpError } from '../platform/errors.ts';
import { Status } from '../platform/http-status.ts';
import * as config from './sandbox/config.ts';
import { PREFIX, ownerTag } from './sandbox/names.ts';
import { WORKERS } from './sandbox/worker.ts';
import { sandboxEnv } from './sandbox/tenancy.ts';
import { reserveBrowser, specFor } from './sandbox/launch.ts';
import { forgetInventory } from './sandbox/inventory.ts';

export { listSandboxBrowsers } from './sandbox/inventory.ts';
export { ecsExternalId } from './sandbox/names.ts';

/**
 * The environment to read settings from: a key's (see tenancy.ts), an env
 * object as given, which is what these took before keys chose runtimes, or,
 * with neither, the deployment's.
 */
const envOf = (keyOrEnv?: string | Record<string, string>) =>
  keyOrEnv && typeof keyOrEnv === 'object' ? keyOrEnv : sandboxEnv(keyOrEnv);

/** True when the key (or env, or deployment) can provision cloud browsers. */
export const isConfigured = (keyOrEnv?: string | Record<string, string>) => config.isConfigured(envOf(keyOrEnv));

/** The settings the key (or env, or deployment) still needs, by their documented names. */
export const missingSettings = (keyOrEnv?: string | Record<string, string>) => config.missingSettings(envOf(keyOrEnv));

/**
 * Browser ids this process created sandboxes for. The browser's own claim
 * about its provider is a courtesy, not a source of truth: an older image
 * does not send one, and a client could say anything. Not persisted, after a
 * restart the upstream lookup by name in removeSandbox is the authority.
 */
const provisioned = new Set();
/** Whether this process created a sandbox for the browser. */
export const isProvisioned = (browserId) => provisioned.has(browserId);

/** The key's runtime and its settings, refusing when no key is given or the settings are incomplete. */
function runtimeFor(apiKey, runtime?) {
  if (!apiKey) throw new HttpError(Status.BAD_REQUEST, 'An API key is required');
  const env = sandboxEnv(apiKey, runtime);
  const settings = config.settings(env);
  if (!settings) throw config.unconfigured(env);
  return { worker: WORKERS[settings.runtime], settings };
}

/**
 * Create one cloud browser. Resolves once the sandbox is starting, the browser
 * enrolls on its own and shows up in the registry within ~90s.
 */
export async function createSandbox({ apiKey, name, persona, browserId }: any = {}) {
  const { worker, settings } = runtimeFor(apiKey);
  browserId ||= await reserveBrowser(apiKey, persona);
  // Cleanup follows the runtime the sandbox was made on, not whatever is configured later.
  await control().update(apiKey, browserId, { cleanup: { kind: 'sandbox', browserId, runtime: worker.id } });
  const made = await worker.create(settings, specFor(settings, { apiKey, name, persona, browserId }));
  await markProvisioned(apiKey, browserId);
  return { browserId, sandboxId: made.id, sandboxName: PREFIX + browserId, runtime: worker.id };
}

/** The browser is starting: note it here and in the control plane, and refresh the key's inventory. */
async function markProvisioned(apiKey, browserId) {
  forgetInventory(ownerTag(apiKey));
  provisioned.add(browserId);
  await control().update(apiKey, browserId, { provisioningActive: false });
}

/**
 * Delete the sandbox backing a browser, on the runtime it was made on (else the
 * key's current one). Returns false if there wasn't one.
 *
 * Ownership is checked against the label, not against the live registry: a
 * sandbox outlives its websocket, and a browser id is not a secret, so a
 * disconnected sandbox must still only be deletable by the key that made it.
 */
export async function removeSandbox(browserId, apiKey, runtime?) {
  const { worker, settings } = runtimeFor(apiKey, runtime);
  const found = await worker.find(settings, PREFIX + browserId);
  if (!found || !ownedBy(found, browserId, apiKey)) return false;
  await found.destroy();
  forgetInventory(ownerTag(apiKey));
  provisioned.delete(browserId);
  return true;
}

/** Whether the key owns this browser's sandbox; throws when its labels name another browser. */
function ownedBy(found, browserId, apiKey) {
  if (found.labels['oya-browser-id'] !== browserId) throw new Error('Sandbox ownership mismatch');
  // Same shape as "no such sandbox", don't confirm existence to a non-owner.
  return found.labels['oya-owner'] === ownerTag(apiKey);
}
