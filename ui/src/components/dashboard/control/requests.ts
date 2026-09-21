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

/** A feed's answer: the fleet summary, or an object holding one list. */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Answer = any;

/** One poll's feeds, in CONTROL_PATHS order: what each is called when it fails, and what its answer becomes. */
const FEEDS: Array<[string, (answer: Answer) => Partial<ControlData>]> = [
  ['the fleet', (answer) => ({ fleet: answer })],
  ['sessions', (answer) => ({ sessions: answer.sessions || [] })],
  ['the audit trail', (answer) => ({ audit: answer.events || [] })],
  ['recordings', (answer) => ({ recordings: answer.recordings || [] })],
  ['providers', (answer) => ({ choices: answer.providers || [] })],
];

/** What one poll loaded. A feed that failed is absent, so the view keeps what it last showed. */
export interface ControlLoad extends Partial<ControlData> {
  /** The feeds that failed, in words, with the first failure's reason; '' when every feed answered. */
  error: string;
}

/** "Could not load recordings and providers: reason", from the feeds that failed. */
function partialError(failed: string[], reason: unknown): string {
  return failed.length ? `Could not load ${failed.join(' and ')}: ${errorMessage(reason)}` : '';
}

/**
 * Loads every Control view's data in parallel. One feed failing (recording
 * storage offline, say) used to blank the whole tab; now the rest still shows,
 * and the failure is named. With nothing loaded at all, it rejects as before.
 */
export async function loadControl(apiKey: string): Promise<ControlLoad> {
  const settled = await Promise.allSettled(CONTROL_PATHS.map((path) => api<Answer>(path, { key: apiKey })));
  const rejected = settled.filter((r) => r.status === 'rejected');
  if (rejected.length === settled.length) throw rejected[0].reason;
  const loaded = settled.map((r, i) => (r.status === 'fulfilled' ? FEEDS[i][1](r.value) : {}));
  const failed = FEEDS.filter((_, i) => settled[i].status === 'rejected').map(([name]) => name);
  return { ...Object.assign({}, ...loaded), error: partialError(failed, rejected[0]?.reason) };
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
