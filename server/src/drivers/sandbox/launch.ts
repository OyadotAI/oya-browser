/**
 * Bringing one cloud browser up, whatever runtime it runs on: reserving it in
 * the control plane and writing the one spec every runtime receives, its name,
 * its labels and the environment it enrols with. No runtime re-derives these.
 */
import { randomUUID } from 'crypto';
import { control } from '../../modules/control/service.ts';
import { QUOTAS } from '../../platform/limits.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { PREFIX, ownerTag, displayName } from './names.ts';
import { SANDBOX_TTL_GRACE_MINUTES } from '../constants.ts';
import type { SandboxSpec } from './worker.ts';

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

/** What every runtime is asked to create for this browser. */
export function specFor(config, browser): SandboxSpec {
  const lifetimeMinutes = config.ttlMinutes + SANDBOX_TTL_GRACE_MINUTES;
  const env = { ...envFor(config, browser), OYA_MAX_LIFETIME_MINUTES: String(lifetimeMinutes) };
  requireSingleLines(env);
  const name = PREFIX + browser.browserId;
  return { name, labels: labelsFor(browser), env, ttlMinutes: config.ttlMinutes, lifetimeMinutes };
}

/** A newline would let a value forge an extra entry in an env file. */
function requireSingleLines(env) {
  if (Object.values(env).some((value) => String(value).includes('\n')))
    throw new HttpError(Status.BAD_REQUEST, 'A cloud browser setting must not contain a newline');
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
