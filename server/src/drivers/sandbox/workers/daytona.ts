/**
 * Daytona sandbox runtime: a sandbox from a snapshot of the browser image,
 * with Daytona's own idle stop and hard TTL. The SDK is loaded lazily so the
 * server runs without @daytona/sdk installed when this runtime is not in use.
 */
import { HttpError } from '../../../platform/errors.ts';
import { Status } from '../../../platform/http-status.ts';
import { setting, unset } from '../names.ts';
import {
  SANDBOX_CREATE_TIMEOUT_S,
  SANDBOX_DELETE_TIMEOUT_S,
  SANDBOX_TTL_GRACE_MINUTES,
  SANDBOX_CLIENT_CACHE_MAX,
} from '../../constants.ts';

/** The runtime's name, as OYA_CLOUD_RUNTIME selects it. */
export const id = 'daytona';

/** A key runs on its own Daytona account once it brings its own API key. */
export const ownAccount = (env) => !!env.OYA_CLOUD_API_KEY;

/** Region used when none is configured. */
const DEFAULT_TARGET = 'us';

/** The sandbox process session the browser entrypoint runs in. */
const SESSION = 'oya-browser';

/** The Daytona settings from env, or null unless the API key and snapshot are set. */
export function settings(env) {
  const apiKey = setting(env, 'API_KEY');
  const snapshot = setting(env, 'SNAPSHOT');
  if (!apiKey || !snapshot) return null;
  return { apiKey, snapshot, ...location(env) };
}

/** Which Daytona API and region. */
const location = (env) => ({
  apiUrl: setting(env, 'API_URL') || null,
  target: setting(env, 'TARGET') || DEFAULT_TARGET,
});

/** What is unset, under the documented names even when the legacy ones are in play. */
export const missing = (env) =>
  unset([
    ['OYA_CLOUD_API_KEY', setting(env, 'API_KEY')],
    ['OYA_CLOUD_SNAPSHOT', setting(env, 'SNAPSHOT')],
  ]);

/** Clients by credential, so keys on their own Daytona accounts each get their own. */
const clients = new Map();

/** The SDK client for these settings (the deployment's by default), created once per credential. */
export async function client(config = settings(process.env)) {
  if (!config) throw new HttpError(Status.CONFLICT, `Daytona needs ${missing(process.env).join(', ')}`);
  const cacheKey = `${config.apiKey}\n${config.apiUrl}\n${config.target}`;
  if (!clients.has(cacheKey)) remember(cacheKey, loadSdk(config, cacheKey));
  return clients.get(cacheKey);
}

/** Caches a client, dropping the oldest past the cap. */
function remember(cacheKey, promise) {
  if (clients.size >= SANDBOX_CLIENT_CACHE_MAX) clients.delete(clients.keys().next().value);
  clients.set(cacheKey, promise);
}

/** Imports the SDK and builds the client; a failed import is forgotten so it can be retried. */
function loadSdk(config, cacheKey) {
  return import('@daytona/sdk')
    .then(({ Daytona }) => new Daytona(clientOptions(config)))
    .catch((err) => {
      clients.delete(cacheKey);
      throw unavailable(err);
    });
}

/** The 409 for a runtime SDK that will not load. */
function unavailable(err) {
  const message = `Oya Cloud runtime unavailable: ${err.message}. Run \`npm i @daytona/sdk\` in server/.`;
  return new HttpError(Status.CONFLICT, message);
}

/** The SDK client's options for these settings. */
function clientOptions(config) {
  return { apiKey: config.apiKey, ...(config.apiUrl ? { apiUrl: config.apiUrl } : {}), target: config.target };
}

/** Whether an SDK error means the sandbox does not exist. */
export function isNotFound(err) {
  return [err?.status, err?.statusCode, err?.response?.status].includes(Status.NOT_FOUND);
}

/** Creates the sandbox, sets its hard TTL and makes sure the browser inside is running. */
export async function create(config, spec) {
  const daytona = await client(config);
  const sandbox = await daytona.create(daytonaSpec(config, spec), { timeout: SANDBOX_CREATE_TIMEOUT_S });
  // Hard cap behind the idle stop, so a wedged sandbox still stops billing.
  await sandbox.setTtl(config.ttlMinutes + SANDBOX_TTL_GRACE_MINUTES);
  await startBrowser(sandbox, config);
  return { id: sandbox.id };
}

/** The spec in Daytona's words. */
const daytonaSpec = (config, spec) => ({
  name: spec.name,
  snapshot: config.snapshot,
  labels: spec.labels,
  envVars: spec.env,
  autoStopInterval: spec.ttlMinutes,
  autoDeleteInterval: 0,
});

/**
 * Legacy snapshots sleep; current images run the browser as their entrypoint.
 * Start it unless it is already running.
 */
async function startBrowser(sandbox, config) {
  await requireBrowserImage(sandbox, config);
  if (await entrypointRunning(sandbox)) return;
  await sandbox.process.createSession(SESSION);
  await sandbox.process.executeSessionCommand(SESSION, { command: 'cd /app && /docker-entrypoint.sh', runAsync: true });
}

/**
 * Check the entrypoint exists first: fired async, a snapshot that is not the
 * Oya browser image fails invisibly and the caller waits out the full enrol
 * window for a browser that was never going to arrive.
 */
async function requireBrowserImage(sandbox, config) {
  const probe = await sandbox.process.executeCommand('test -x /docker-entrypoint.sh && echo ok').catch(() => null);
  if (/\bok\b/.test(probe?.result ?? probe?.output ?? '')) return;
  await sandbox.delete().catch(() => {});
  throw new HttpError(
    Status.CONFLICT,
    `OYA_CLOUD_SNAPSHOT "${config.snapshot}" has no /docker-entrypoint.sh, so it is not an Oya browser image. ` +
      'Build one from browser/Dockerfile, push it, and point OYA_CLOUD_SNAPSHOT at that.',
  );
}

/** Whether the image's own entrypoint is already running the browser. */
async function entrypointRunning(sandbox) {
  const entrypoint = await sandbox.process.getEntrypointSession();
  return entrypoint.commands?.some(
    (command) => command.command?.includes('/docker-entrypoint.sh') && command.exitCode == null,
  );
}

/** The sandbox of this name, or null when Daytona has none. */
export async function find(config, name) {
  try {
    return foundSandbox(await (await client(config)).get(name));
  } catch (err) {
    if (isNotFound(err)) return null;
    throw err;
  }
}

/** A Daytona sandbox as the facade sees one. */
const foundSandbox = (sandbox) => ({
  labels: sandbox.labels || {},
  state: sandbox.state,
  destroy: () => sandbox.delete(SANDBOX_DELETE_TIMEOUT_S, true),
});

/** Every browser sandbox Daytona holds labelled for this owner. */
export async function list(config, owner) {
  const rows = [];
  for await (const sandbox of (await client(config)).list({ labels: { 'oya-browser': 'true', 'oya-owner': owner } })) {
    rows.push({ labels: sandbox.labels || {}, state: sandbox.state });
  }
  return rows;
}
