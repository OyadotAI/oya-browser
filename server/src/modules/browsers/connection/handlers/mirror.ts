/**
 * The desktop mirrors the user's real browser: one persona per profile, each
 * carrying that profile's captured device and its cookies and localStorage, so
 * the desktop and every remote browser on the persona are already signed in.
 *
 * This is a trust boundary. A desktop is sending identity and session data, so
 * the payload is validated and capped before any of it becomes a persona.
 */
import { container } from '../../../../app/container.ts';
import { mergeDump, mergeStorage } from '../../../personas/cookies.ts';
import { MAX_MIRROR_PROFILES, MAX_MIRROR_COOKIES } from '../constants.ts';
import type { Handler } from './types.ts';

/** One profile as the desktop sends it. */
interface MirrorMsg {
  /** The source browser, e.g. "chrome". */
  source: string;
  /** The profile directory, e.g. "Default". */
  profile: string;
  /** Display name for the persona. */
  name?: string;
  /** Whether this is the last-used profile, and so the desktop's default. */
  lastUsed?: boolean;
  /** The captured real-device fingerprint. */
  device: unknown;
  /** The profile's cookies, in the pool's slim shape. */
  cookies?: unknown[];
  /** The profile's localStorage, by origin. */
  origins?: Record<string, unknown>;
}

/** A device object must at least name a platform and a screen; anything less is not a device. */
function validDevice(device: unknown): device is Record<string, any> {
  const d = device as any;
  return !!d && typeof d === 'object' && !!d.navigator?.platform && !!d.screen;
}

/** Why this payload cannot be mirrored, or null when it is a bounded array of well-formed profiles. */
function rejection(msg: any): string | null {
  if (!Array.isArray(msg.profiles) || !msg.profiles.length) return 'mirror_persona needs a non-empty profiles array';
  if (msg.profiles.length > MAX_MIRROR_PROFILES) return `At most ${MAX_MIRROR_PROFILES} profiles per import`;
  for (const p of msg.profiles) {
    if (typeof p?.source !== 'string' || typeof p?.profile !== 'string')
      return 'Each profile needs a source and profile';
    if (!validDevice(p.device)) return 'Each profile needs a device';
  }
  return null;
}

/** Turns one validated profile into a persona and seeds its jar. Returns the persona id. */
function mirrorOne(apiKey: string, p: MirrorMsg): string {
  const persona = container.personas.mirror(apiKey, p);
  if (Array.isArray(p.cookies)) mergeDump(persona.id, p.cookies.slice(0, MAX_MIRROR_COOKIES));
  if (p.origins && typeof p.origins === 'object') mergeStorage(persona.id, p.origins);
  return persona.id;
}

/** Mirrors every profile and picks the default: the last-used one, or the first if none says so. */
function mirrorAll(apiKey: string, profiles: MirrorMsg[]) {
  const personaIds = profiles.map((p) => mirrorOne(apiKey, p));
  const lastUsed = profiles.findIndex((p) => p.lastUsed);
  return { personaIds, defaultPersonaId: personaIds[lastUsed >= 0 ? lastUsed : 0] };
}

/**
 * Create the personas, seed each jar, and answer with the default: the
 * last-used profile's persona, so the desktop connects as it next time.
 */
export const mirrorPersona: Handler = (conn, msg) => {
  const reason = rejection(msg);
  if (reason) return void (conn.isOpen() && conn.send({ type: 'mirror_failed', error: reason }));
  const result = mirrorAll(conn.apiKey, msg.profiles as MirrorMsg[]);
  console.log(
    `[ws] Mirror from ${conn.browserId}: ${result.personaIds.length} profiles, default ${result.defaultPersonaId}`,
  );
  if (conn.isOpen()) conn.send({ type: 'mirror_ok', ...result });
};
