/**
 * The Control tab's calls to the server. Everything is scoped to the API key
 * it is given: there is no admin tier, the key is the identity.
 */
import { api, errorMessage } from '@/lib/api-client';
import { apiUrl } from '@/lib/api';
import { CONTROL_PATHS } from './constants';
import type { AuditEvent, ControlFleet, Frame, ProviderChoice, Recording, Session } from './types';

/** Everything the Control tab shows, from one poll. */
export interface ControlData {
  /** Fleet summary, once loaded. */
  fleet: ControlFleet | null;
  /** CDP gateway sessions. */
  sessions: Session[];
  /** Audit trail. */
  audit: AuditEvent[];
  /** Recorded sessions. */
  recordings: Recording[];
  /** Vendors and whether each has a saved credential. */
  choices: ProviderChoice[];
}

/** Nothing loaded yet. */
export const EMPTY_CONTROL: ControlData = { fleet: null, sessions: [], audit: [], recordings: [], choices: [] };

/** Loads every Control view's data in parallel. */
export async function loadControl(apiKey: string): Promise<ControlData> {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const get = (path: string) => api<any>(path, { key: apiKey });
  const [f, s, a, rec, providerOptions] = await Promise.all(CONTROL_PATHS.map(get));
  const lists = { sessions: s.sessions || [], audit: a.events || [], recordings: rec.recordings || [] };
  return { fleet: f, choices: providerOptions.providers || [], ...lists };
}

/** POSTs to the gateway API; an empty body is sent as none. */
export const post = (apiKey: string, path: string, body?: unknown) =>
  api(path, { key: apiKey, method: 'POST', body: body || undefined });

/** DELETEs a gateway resource. */
export const del = (apiKey: string, path: string) => api(path, { key: apiKey, method: 'DELETE' });

/** Waits for `work`; resolves to '' on success or the failure's message. */
export async function failure(work: () => Promise<unknown>, fallback: string): Promise<string> {
  try {
    await work();
    return '';
  } catch (e) {
    return errorMessage(e, fallback);
  }
}

/** A recording's frame list. */
export const fetchFrames = (sessionId: string, headers: HeadersInit): Promise<Frame[]> =>
  fetch(apiUrl(`/gateway/recordings/${sessionId}`), { headers })
    .then((r) => r.json())
    .then((m) => m.frames || []);

/** One recording frame as an image blob. */
export const fetchFrame = (sessionId: string, i: number, headers: HeadersInit): Promise<Blob> =>
  fetch(apiUrl(`/gateway/recordings/${sessionId}/frames/${i}`), { headers }).then((r) =>
    r.ok ? r.blob() : Promise.reject(new Error('frame')),
  );

/** Runs `start` with a liveness check; the returned cleanup makes late results stale. */
export function whileMounted(start: (live: () => boolean) => void) {
  let cancelled = false;
  start(() => !cancelled);
  return () => {
    cancelled = true;
  };
}
