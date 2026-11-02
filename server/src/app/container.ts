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
import { captureUnexpected } from '../platform/sentry.ts';
import { BEARER_PREFIX_LENGTH } from '../platform/constants.ts';
import { track } from '../modules/telemetry/index.ts';
import { fingerprint as ownerOf } from '../platform/audit.ts';
import { dataPath } from '../platform/paths.ts';
import * as proxies from '../modules/proxies/service.ts';
import * as mfa from '../modules/challenges/mfa.ts';
import * as credentials from '../modules/personas/credentials.ts';
import * as logins from '../modules/personas/cookies.ts';
import {
  GroupService,
  GroupTable,
  PersonaService,
  PersonaTable,
  type GroupRepository,
  type PersonaRepository,
} from '../modules/personas/index.ts';
import { createBilling } from '../modules/billing/index.ts';
import { claimUrl, getKeyOwner, isUnclaimedAgentKey, keyDigest, keysOf } from '../modules/auth/service.ts';
import { registry } from '../modules/browsers/registry.ts';
import { SERVER_RUN } from '../modules/browsers/connection/constants.ts';
import { consoleUrl } from '../modules/slack/service.ts';
import * as keyConfig from '../modules/config/service.ts';
import * as usage from '../platform/usage.ts';
import * as license from '../platform/license/index.ts';
import { audit } from '../platform/audit.ts';
import * as profiles from '../modules/gateway/profiles.ts';
import { control } from '../modules/control/service.ts';
import { Erasure, erasureRows } from '../modules/erasure/index.ts';

/** Personas in the configured storage, taking in a personas.json left from before storage drivers. */
function personaRepository(): PersonaRepository {
  return new PersonaTable(dataPath('personas.json'));
}

/** What the persona service calls besides its repository and the groups. */
const PERSONA_MODULES = { ownerOf, proxies, mfa, credentials, logins, metrics };

/** The persona service, wired to its repository, the groups its personas leave, and the modules it calls. */
function personaService(groups: GroupRepository) {
  return new PersonaService({ repository: personaRepository(), ...PERSONA_MODULES, groups });
}

/** The persona group service, over the same group table the persona service drops members from. */
function groupService(repository: GroupRepository, personas: PersonaService) {
  return new GroupService({ repository, personas, ownerOf });
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
  unclaimed: (key: string) => isUnclaimedAgentKey(key),
  stepsOfKey: async (key: string, hours: number) =>
    (await usage.history(key, { hours })).rows.reduce((sum, row) => sum + (Number(row.agent_steps) || 0), 0),
  claimUrl: (key: string) => claimUrl(key),
  ownsModel: (key: string) => Boolean(keyConfig.resolve(key).own),
  modelOf: (key: string) => keyConfig.resolve(key).model,
};

/** The self-hosted license, as billing asks it. */
const LICENSE = {
  licenseAdmit: (count: number) => license.admit(count),
  hostedDeployment: () => license.hostedDeployment(),
};

/** Reports an unexpected failure as a product event and to Sentry, so a 500 shows up where the owner looks. */
function reportUnexpected(ref: string, req: Asked, err: unknown) {
  const key = String(req.headers?.authorization || '').slice(BEARER_PREFIX_LENGTH) || null;
  const where = { ref, method: req.method || '', route: req.route?.path ?? 'middleware' };
  track.serverError(key, where);
  captureUnexpected(err, where);
}

/** Erasure of deleted projects and accounts, wired to everything that holds a project's data. */
function erasure(personas: PersonaService) {
  return new Erasure({
    ...{ store: () => control().store, rows: erasureRows, personas, settings: keyConfig, usage, profiles, audit },
    deleteProject: (userId, id) => control().updateOwnedProject(userId, id, { remove: true, stopBrowsers: true }),
  });
}

/** Build every service once, wired to its dependencies. */
export function createContainer() {
  const groupTable = new GroupTable();
  const personas = personaService(groupTable);
  personas.startAutosave();
  setUnexpectedReporter(reportUnexpected);
  return { personas, groups: groupService(groupTable, personas), billing: billing(), erasure: erasure(personas) };
}

/** The services the composition root provides. */
export type Container = ReturnType<typeof createContainer>;

/** The process-wide composition root. */
export const container: Container = createContainer();
