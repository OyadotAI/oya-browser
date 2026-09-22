/**
 * Who an event is about. A key's owner lives in Supabase and is looked up
 * once an hour at most, never on the request path: the seam gets a short
 * fingerprint label at once and the owner's email catches up in the background,
 * so a request is never slowed by analytics.
 */
import { fingerprint } from '../../platform/audit.ts';
import { getKeyOwner, getProfile } from '../auth/service.ts';
import { WHO_CACHE_MAX, WHO_FINGERPRINT_CHARS, WHO_TTL_MS } from './constants.ts';
import type { Who } from './catalog.ts';

/** A remembered owner, with when it was learned. */
type Remembered = {
  /** The owner, or null for a key nobody owns. */
  who: Who | null;
  /** When it was learned. */
  at: number;
};

/** Owners by key fingerprint. */
const remembered = new Map<string, Remembered>();

/** The label a key gets when nobody owns it: `key ` and a short fingerprint. */
export const keyLabel = (key: string) => `key ${fingerprint(key).slice(0, WHO_FINGERPRINT_CHARS)}`;

/** A person as an event is about them. */
export const person = (userId: string, email?: string | null): Who => ({
  id: userId,
  email: email || undefined,
  label: email || userId,
});

/** A key with no known owner, for events that are only about the key. */
export const anonymous = (key: string): Who => ({ id: fingerprint(key), label: keyLabel(key) });

/** Nobody: an event with no caller behind it, such as a 500 before authentication. */
export const nobody: Who = { id: 'unauthenticated', label: '' };

/** What is remembered about a key's owner, or undefined when nothing fresh is. */
function fresh(fp: string) {
  const hit = remembered.get(fp);
  return hit && Date.now() - hit.at < WHO_TTL_MS ? hit.who : undefined;
}

/** Remembers an owner, forgetting the oldest once the cache is full. */
function remember(fp: string, who: Who | null) {
  if (remembered.size >= WHO_CACHE_MAX) remembered.delete(remembered.keys().next().value);
  remembered.set(fp, { who, at: Date.now() });
}

/** The key's owner as a person, or null when nobody owns it. A profile that cannot be read still leaves the person, only nameless. */
async function ownerOf(key: string): Promise<Who | null> {
  const userId = await getKeyOwner(key);
  if (!userId) return null;
  const email = await getProfile(userId)
    .then((profile) => profile?.email)
    .catch(() => undefined);
  return person(userId, email);
}

/** Looks the owner up and remembers the answer; any failure means no owner, for an hour. */
async function lookup(key: string, fp: string): Promise<Who | null> {
  const who = await ownerOf(key).catch(() => null);
  remember(fp, who);
  return who;
}

/**
 * Who a key belongs to: at once when remembered, otherwise the key itself now
 * and the owner once the lookup lands. Callers never wait on it.
 */
export function whoHolds(key: string): Who | Promise<Who> {
  const fp = fingerprint(key);
  const known = fresh(fp);
  if (known !== undefined) return known ?? anonymous(key);
  // Remembered as unowned before the lookup lands, so a burst of events for a
  // cold key starts one lookup, not one per event: a replica restart must not
  // turn a thousand reconnects into a thousand pairs of Supabase reads.
  remember(fp, null);
  return lookup(key, fp).then((who) => who ?? anonymous(key));
}

/** Forgets every owner, so one test cannot leak into the next. */
export const forgetAllForTests = () => remembered.clear();
