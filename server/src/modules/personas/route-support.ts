/**
 * Pieces every persona route shares: the ownership lookup that answers 404,
 * the audit record, and the refusal of device choices that cannot be honoured.
 */
import type { Request, Response } from 'express';
import { audit } from '../../platform/audit.ts';
import { Status } from '../../platform/http-status.ts';
import { getKey } from '../../app/http.ts';
import type { PersonaService } from './service.ts';
import { prefsError } from './fingerprint.ts';

/** The answer for a persona id the caller does not own, or that does not exist. */
export const NO_SUCH_PERSONA = 'No such persona';

/** Answers 404 for a missing or unowned persona. */
export const notFound = (res: Response) => res.status(Status.NOT_FOUND).json({ error: NO_SUCH_PERSONA });

/** The caller's persona named by `:id`, or null after answering 404. */
export function ownedPersona(personas: PersonaService, req: Request, res: Response) {
  const p = personas.get(getKey(req), req.params.id as string);
  if (!p) notFound(res);
  return p;
}

/** What an audit record may add beyond who did what to which persona. */
type AuditExtra = {
  /** Small, non-secret detail. */
  meta?: object;
  /** 'ok' or 'error'. */
  outcome?: string;
};

/** Records an action on a persona, by the calling key. */
export function auditPersona(req: Request, action: string, targetId: string, extra: AuditExtra = {}) {
  audit({ action, actorKey: getKey(req), targetType: 'persona', targetId, ...extra, req });
}

/** Answers 400 when the body's prefs ask for a device that is not offered; true if it did. */
export function refusedPrefs(req: Request, res: Response) {
  const invalid = prefsError(req.body?.prefs);
  if (invalid) res.status(Status.BAD_REQUEST).json({ error: invalid });
  return !!invalid;
}
