/**
 * The self-hosted license, as the server sees it: how many cloud browsers may
 * run at once, and the daily ping that tells Oya the install exists. The work
 * is done by a vendored module; this facade only loads it and asks it. When that module is missing or will not load, the
 * facade holds the free cap itself: a broken install never runs unlicensed
 * past it.
 */
import { LICENSE_EMAIL, PING_URL, SELF_HOST_FREE_CAP, VENDOR_FILE } from './constants.ts';
import * as store from './repository.ts';
import { HttpError } from '../errors.ts';
import { Status } from '../http-status.ts';

export { SELF_HOST_FREE_CAP, LICENSE_EMAIL } from './constants.ts';

/** A decision on a cloud start. */
export type Admission = {
  /** Whether it may start. */
  ok: boolean;
  /** Why not, when it may not. */
  message: string;
};

/** What the server hands the licensing module when it starts. */
export type LicenseDeps = {
  /** The release this server runs. */
  version: string;
  /** How many browsers are connected now. */
  browsers(): number;
};

/** The vendored module's surface. */
export type LicenseModule = {
  /** Loads the license key and install id and schedules the daily ping. */
  start(deps: Record<string, unknown>): void | Promise<void>;
  /** Whether `count` cloud browsers at once is covered. */
  admit(count: number): Admission;
  /** Whether this server runs under Oya's hosted-deployment license. */
  isHosted?(): boolean;
  /** Signs a new license with the configured key. */
  mint?(request: Record<string, unknown>, signingKey: string): Minted;
};

/** A license just issued, and its key. */
export type Minted = {
  /** What the license says. */
  license: {
    /** Its id. */
    id: string;
    /** Who it is for. */
    licensee: string;
    /** Cloud browsers at once. */
    maxConcurrent: number;
    /** When it ends, ISO. */
    expiresAt: string;
  };
  /** The key the licensee sets as OYA_LICENSE_KEY. */
  token: string;
};

/** The loaded module, or null when there is none. */
let impl: LicenseModule | null = null;

/** What an unlicensed server over the cap is told. */
export const overCap = (cap: number) =>
  `Self-hosted use above ${cap} concurrent cloud browsers needs a commercial license: ${LICENSE_EMAIL}`;

/** Loads the vendored module; a missing or broken one leaves the free cap in force. */
export async function load(file = VENDOR_FILE) {
  impl = await import(file).then((mod) => (mod.default ?? mod) as LicenseModule, notLoaded);
}

/** Says the module did not load, and leaves none in place. */
function notLoaded(e: Error) {
  console.warn(`[license] licensing module not loaded (${e.message}); cloud browsers capped at ${SELF_HOST_FREE_CAP}`);
  return null;
}

/** Whether `count` cloud browsers at once is covered: the module decides, or the free cap without it. */
export function admit(count: number): Admission {
  if (impl) return impl.admit(count);
  return { ok: count <= SELF_HOST_FREE_CAP, message: overCap(SELF_HOST_FREE_CAP) };
}

/** Whether the license says this is Oya's own hosted deployment; never without the module. */
export const hostedDeployment = () => Boolean(impl?.isHosted?.());

/** Starts the module: license key, install id, ping. Never throws; a ping that cannot go out changes nothing. */
export async function start(deps: LicenseDeps) {
  const context = { ...deps, licenseKey: process.env.OYA_LICENSE_KEY || '', pingUrl: PING_URL, store, fetch };
  await Promise.resolve(impl?.start({ ...context, freeCap: SELF_HOST_FREE_CAP, overCap })).catch(() => {});
}

/** Issues a license, where this server holds the signing key (OYA_LICENSE_SIGNING_KEY); a bad request is a 400. */
export function mint(request: Record<string, unknown>): Minted {
  const key = process.env.OYA_LICENSE_SIGNING_KEY || '';
  if (!impl?.mint || !key) throw new HttpError(Status.UNAVAILABLE, 'Issuing licenses is not set up on this server.');
  try {
    return impl.mint(request, key);
  } catch (e) {
    throw new HttpError(Status.BAD_REQUEST, e.message, { code: 'invalid_request' });
  }
}

/** Test hook: puts a fake module in place, or none. */
export function setModuleForTests(fake: LicenseModule | null) {
  impl = fake;
}
