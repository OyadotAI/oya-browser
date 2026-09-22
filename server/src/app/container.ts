/**
 * The composition root: every layered module's service is built here, once,
 * with its dependencies passed in. Nothing below app/ constructs a service or
 * reaches for a singleton; it takes what it needs in its constructor.
 *
 * Hand-wired rather than a DI library: decorator-based containers need a
 * transpiler, and the server runs its TypeScript as-is. The types check the
 * wiring instead.
 */
import { db } from '../platform/db.ts';
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
import {
  PersonaService,
  FilePersonaRepository,
  SupabasePersonaRepository,
  FallbackPersonaRepository,
  type PersonaRepository,
} from '../modules/personas/index.ts';

/** Supabase-backed personas with a local file fallback when a database is configured; the file alone otherwise. */
function personaRepository(): PersonaRepository {
  const file = new FilePersonaRepository(dataPath('personas.json'));
  return db ? new FallbackPersonaRepository(new SupabasePersonaRepository(db), file) : file;
}

/** The persona service, wired to its repository and the modules it calls. */
function personaService() {
  return new PersonaService({ repository: personaRepository(), ownerOf, proxies, mfa, credentials, logins, metrics });
}

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
  return { personas };
}

/** The services the composition root provides. */
export type Container = ReturnType<typeof createContainer>;

/** The process-wide composition root. */
export const container: Container = createContainer();
