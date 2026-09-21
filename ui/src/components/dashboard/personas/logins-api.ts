/**
 * A profile's cookie jar over the API, and the file formats it moves in: what
 * "Move logins" in the profile drawer is made of.
 */
import { api } from '@/lib/api-client';
import type { Persona } from '../types';
import { JSON_INDENT } from './constants';

/** A cookie as the jar holds it. */
export type Cookie = Record<string, unknown>;

/** What the server answers to an import. */
export interface Imported {
  /** Cookies merged. */
  imported: number;
  /** Cookies left out as unusable or expired. */
  skipped: number;
  /** Cookies in the jar afterwards. */
  total: number;
}

/** `GET /pool/cookies`. */
interface Jar {
  /** The profile's cookies. */
  cookies: Cookie[];
}

/** `GET /personas`. */
interface Listing {
  /** Every profile on the key. */
  personas: Persona[];
}

/** A profile's jar path. */
const jarPath = (id: string) => `/pool/cookies?persona=${encodeURIComponent(id)}`;

/** The cookies of a profile. */
export async function fetchJar(key: string, id: string): Promise<Cookie[]> {
  const jar = await api<Jar>(`${jarPath(id)}&format=json`, { key });
  return jar.cookies;
}

/** The key's profiles other than `id`, to copy logins from; a failure leaves the list empty. */
export async function fetchOtherProfiles(key: string, id: string): Promise<Persona[]> {
  const listing = await api<Listing>('/personas', { key }).catch(() => ({ personas: [] }));
  return (listing.personas || []).filter((p) => p.id !== id);
}

/** Merges cookies into a profile's jar. */
export const putJar = (key: string, id: string, cookies: Cookie[]) =>
  api<Imported>(jarPath(id), { key, method: 'PUT', body: { cookies } });

/** Saves `cookies` as a JSON file named after the profile. */
export function downloadJar(name: string, cookies: Cookie[]) {
  const url = URL.createObjectURL(new Blob([JSON.stringify(cookies, null, JSON_INDENT)], { type: 'application/json' }));
  const link = Object.assign(document.createElement('a'), { href: url, download: `${name}-logins.json` });
  link.click();
  URL.revokeObjectURL(url);
}

/** The cookies in a file's text: a list, or an export wrapped as `{ cookies }`. */
export function cookiesIn(text: string): Cookie[] {
  const parsed = JSON.parse(text);
  const list = Array.isArray(parsed) ? parsed : parsed?.cookies;
  if (!Array.isArray(list)) throw new Error('That file holds no list of cookies.');
  return list;
}

/** What an import did, in words. */
export const mergedNote = (r: Imported) =>
  `${r.imported} imported, ${r.skipped} skipped. ${r.total} cookies in this profile now.`;

/** What an export did, in words, with why the file matters. */
export const exportedNote = (count: number) =>
  `${count} cookie${count === 1 ? '' : 's'} exported. Keep the file private: it holds live sessions.`;
