/**
 * The composition root: every layered module's service is built here, once,
 * with its dependencies passed in. Nothing below app/ constructs a service or
 * reaches for a singleton; it takes what it needs in its constructor.
 *
 * Hand-wired rather than a DI library: decorator-based containers need a
 * transpiler, and the server runs its TypeScript as-is. The types check the
 * wiring instead.
 */
import { metrics } from '../platform/metrics.ts';
import { setUnexpectedReporter, type Asked } from '../platform/errors.ts';
import { BEARER_PREFIX_LENGTH } from '../platform/constants.ts';
import { track } from '../modules/telemetry/index.ts';
import { fingerprint as ownerOf } from '../platform/audit.ts';
import { dataPath } from '../platform/paths.ts';
import * as proxies from '../modules/proxies/service.ts';
import * as mfa from '../modules/challenges/mfa.ts';
import * as credentials from '../modules/personas/credentials.ts';
import * as logins from '../modules/personas/cookies.ts';
import { PersonaService, PersonaTable, type PersonaRepository } from '../modules/personas/index.ts';
import { createBilling } from '../modules/billing/index.ts';
import { getKeyOwner, keyDigest, keysOf } from '../modules/auth/service.ts';
import { registry } from '../modules/browsers/registry.ts';
import { SERVER_RUN } from '../modules/browsers/connection/constants.ts';
import { consoleUrl } from '../modules/slack/service.ts';
import * as keyConfig from '../modules/config/service.ts';
import * as usage from '../platform/usage.ts';
import * as license from '../platform/license/index.ts';

/** Personas in the configured storage, taking in a personas.json left from before storage drivers. */
function personaRepository(): PersonaRepository {
  return new PersonaTable(dataPath('personas.json'));
}

/** The persona service, wired to its repository and the modules it calls. */
function personaService() {
  return new PersonaService({ repository: personaRepository(), ownerOf, proxies, mfa, credentials, logins, metrics });
}

/** The key of every cloud browser this replica runs. */
function cloudKeys(): string[] {
  return [...registry.browsers.values()].filter((b: any) => SERVER_RUN.has(b.provider)).map((b: any) => b.apiKey);
}

/**
 * Billing, wired to who owns which key, what runs, and the license. Every
 * dependency is called late: these modules import the container back.
 */
function billing() {
  return createBilling({ ...KEYS, ...LICENSE, cloudKeys, consoleUrl: () => consoleUrl() });
}

/** Who owns a key, what its model is, and whose usage it books to. */
const KEYS = {
  ownerOf: (key: string) => getKeyOwner(key),
  keyDigest: (key: string) => keyDigest(key),
  keyDigestsOf: async (userId: string) => new Set((await keysOf(userId)).map((row) => String(row.key_hash))),
  billTo: (key: string, userId: string) => usage.billTo(key, userId),
  ownsModel: (key: string) => Boolean(keyConfig.resolve(key).own),
  modelOf: (key: string) => keyConfig.resolve(key).model,
};

/** The self-hosted license, as billing asks it. */
const LICENSE = {
  licenseAdmit: (count: number) => license.admit(count),
  hostedDeployment: () => license.hostedDeployment(),
};

/** Reports an unexpected failure as a product event, so a 500 shows up where the owner looks. */
function reportUnexpected(ref: string, req: Asked) {
  const key = String(req.headers?.authorization || '').slice(BEARER_PREFIX_LENGTH) || null;
  track.serverError(key, { ref, method: req.method || '', route: req.route?.path ?? 'middleware' });
}

/** Build every service once, wired to its dependencies. */
export function createContainer() {
  const personas = personaService();
  personas.startAutosave();
  setUnexpectedReporter(reportUnexpected);
  return { personas, billing: billing() };
}

/** The services the composition root provides. */
export type Container = ReturnType<typeof createContainer>;

/** The process-wide composition root. */
export const container: Container = createContainer();
