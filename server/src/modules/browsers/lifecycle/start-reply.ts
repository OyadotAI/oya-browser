/**
 * One start request as it moves through the launchers, and the answer each
 * launcher gives once its browser exists.
 */
import { audit } from '../../../platform/audit.ts';
import { Status } from '../../../platform/http-status.ts';

/** A start in progress. */
export interface Start {
  /** The request. */
  req: any;
  /** Its response. */
  res: any;
  /** The caller's key. */
  key: string;
  /** The provider asked for, or the key's configured one. */
  wanted: string;
  /** The persona it runs as. */
  persona: any;
}

/** Audits the start and answers 201 with `body`. */
export function started({ req, res, key, persona }: Start, body) {
  // `reused` keeps the trail honest: nothing was started, one was borrowed.
  const meta = { provider: body.provider, persona: persona.id, ...(body.reused ? { reused: true } : {}) };
  audit({ action: 'browser.start', actorKey: key, targetType: 'browser', targetId: body.id, meta, req });
  res.status(Status.CREATED).json(body);
}

/** The body for a browser that is still booting and will dial in. */
export const starting = (start: Start, id: string, extra: object) => ({
  id,
  provider: start.wanted,
  persona: start.persona.id,
  status: 'starting',
  ...extra,
});
