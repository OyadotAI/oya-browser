/**
 * What a stop reports: its audit record and the not-connected answer.
 */
import { audit } from '../../../platform/audit.ts';

/** What stopping one browser reports. */
export type StopResult = {
  /** The browser asked about. */
  id: string;
  /** Whether it is stopped (or stopping). */
  ok: boolean;
  /** A durable state, or the HTTP status a failure should be answered with. */
  status?: string | number;
  /** Why it could not be stopped. */
  error?: string;
  /** Whether an Oya Cloud sandbox was removed; null when none was looked for. */
  sandboxRemoved?: boolean | null;
  /** Who ran it. */
  provider?: string;
};

/** The answer for a browser the caller cannot see or that is not there. */
export const notConnected = (browserId) => ({ id: browserId, ok: false, error: 'Browser not connected' });

/** Audits a stop of `browserId` with `meta`. */
export function auditStop(req, key, browserId, meta) {
  audit({ action: 'browser.stop', actorKey: key, targetType: 'browser', targetId: browserId, meta, req });
}
