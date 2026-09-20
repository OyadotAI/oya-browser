/**
 * Bringing one cloud browser up: reserving it in the control plane, creating
 * its sandbox, and making sure the browser inside is running.
 */
import { randomUUID } from 'crypto';
import { control } from '../../modules/control/service.ts';
import { QUOTAS } from '../../platform/limits.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { PREFIX, ownerTag, displayName } from './config.ts';
import { SANDBOX_CREATE_TIMEOUT_S, SANDBOX_TTL_GRACE_MINUTES } from '../constants.ts';

/** The sandbox process session the browser entrypoint runs in. */
const SESSION = 'oya-browser';

/**
 * CDP front door for Playwright and friends, reached only through the
 * control socket relay (cdp-relay.js). Loopback: no port leaves the sandbox.
 */
const CDP_FRONT_DOOR = { OYA_REMOTE_DEBUGGING_PORT: '9222', OYA_REMOTE_DEBUGGING_HOST: '127.0.0.1' };

/** Environment every cloud browser gets. */
const CLOUD_ENV = {
  // So the control plane knows a Stop must destroy this sandbox.
  OYA_PROVIDER: 'oya-cloud',
  ...CDP_FRONT_DOOR,
};

/** Reserves a new browser id in the control plane, within the key's quotas. */
export async function reserveBrowser(apiKey, persona) {
  const browserId = randomUUID();
  await control().reserve(apiKey, reservation(browserId, persona));
  return browserId;
}

/** The control-plane reservation for a managed cloud browser. */
function reservation(browserId, persona) {
  const limits = { maxConcurrent: QUOTAS.browsers, hourlyLimit: QUOTAS.sandboxesPerHour };
  return { id: browserId, provider: 'oya-cloud', persona, ...limits, managed: true };
}

/** Creates the sandbox and sets its hard TTL. */
export async function launch(daytona, config, browser) {
  const sandbox = await daytona.create(sandboxSpec(config, browser), { timeout: SANDBOX_CREATE_TIMEOUT_S });
  // Hard cap behind the idle stop, so a wedged sandbox still stops billing.
  await sandbox.setTtl(config.ttlMinutes + SANDBOX_TTL_GRACE_MINUTES);
  return sandbox;
}

/** What the runtime is asked to create. */
function sandboxSpec(config, browser) {
  return {
    name: PREFIX + browser.browserId,
    snapshot: config.snapshot,
    labels: labelsFor(browser),
    envVars: envFor(config, browser),
    autoStopInterval: config.ttlMinutes,
    autoDeleteInterval: 0,
  };
}

/** Labels that find the sandbox again and prove who owns it. */
function labelsFor({ apiKey, name, persona, browserId }) {
  return {
    'oya-browser': 'true',
    'oya-browser-id': browserId,
    'oya-owner': ownerTag(apiKey),
    'oya-name': displayName(name, browserId),
    ...(persona ? { 'oya-persona': persona } : {}),
  };
}

/** The contract browser/main.js reads to enrol over the normal WebSocket. */
function envFor(config, browser) {
  return {
    ...enrolment(config, browser),
    // Which identity it runs as: fingerprint, cookie jar and proxy together.
    ...(browser.persona ? { OYA_PERSONA: browser.persona } : {}),
    ...CLOUD_ENV,
  };
}

/** Where to enrol and as what. */
function enrolment(config, { apiKey, name, browserId }) {
  return {
    OYA_SERVER_URL: config.wsUrl,
    OYA_API_KEY: apiKey,
    OYA_BROWSER_ID: browserId,
    OYA_BROWSER_NAME: displayName(name, browserId),
    OYA_AUTO_CONNECT: 'true',
  };
}

/**
 * Legacy snapshots sleep; current images run the browser as their entrypoint.
 * Start it unless it is already running.
 */
export async function startBrowser(sandbox, config) {
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
