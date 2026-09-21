/**
 * The switcher's requests: account-token calls to /auth/projects, and the
 * project id an API key opens.
 */
import { apiUrl, authHeaders, listApiKeys } from '@/lib/api';
import { GONE_STATUSES, HEX_BYTE_WIDTH, HEX_RADIX, PROJECT_ID_HEX_CHARS } from './constants';
import type { Failure, OwnedKey } from './types';

/** An error's message, or `fallback` for anything that is not an Error. */
export const message = (e: unknown, fallback = 'Something went wrong') => (e instanceof Error ? e.message : fallback);

/** Whether a failure means the account can no longer open the project. */
export const isGone = (e: unknown) => GONE_STATUSES.includes((e as Failure).status ?? 0);

/** The error when the server gives no reason. */
const FAILED = 'Project action failed';

/** One call with the account token; a failure carries the server's reason, status and code. */
export async function call<T>(token: string, path: string, init: RequestInit = {}, fallback = FAILED): Promise<T> {
  const res = await fetch(apiUrl(path), { ...init, headers: authHeaders(token) });
  // A proxy error page is not JSON; its status still says what happened.
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw Object.assign(new Error(data.error || fallback), { status: res.status, code: data.code });
  return data;
}

/** A call on one project: `/auth/projects/:id` plus `suffix`. */
export function projectRequest<T>(token: string, id: string, method: string, body?: object, suffix = '') {
  const init = { method, ...(body ? { body: JSON.stringify(body) } : {}) };
  return call<T>(token, `/auth/projects/${encodeURIComponent(id)}${suffix}`, init);
}

/** The keys the account owns; a failure just leaves the prefixes out. */
export function listOwnedKeys(token: string): Promise<OwnedKey[]> {
  return listApiKeys(token)
    .then((d) => (Array.isArray(d) ? d : (d?.keys ?? [])) as OwnedKey[])
    .catch(() => [] as OwnedKey[]);
}

/** The project an API key opens: the server derives it the same way (service.js projectId). */
export async function projectIdFor(key: string) {
  const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(key)));
  const hex = Array.from(digest, (b) => b.toString(HEX_RADIX).padStart(HEX_BYTE_WIDTH, '0')).join('');
  return `prj_${hex.slice(0, PROJECT_ID_HEX_CHARS)}`;
}
