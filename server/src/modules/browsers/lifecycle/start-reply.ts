/**
 * One start request as it moves through the launchers, and the answer each
 * launcher gives once its browser exists.
 */
import { audit } from '../../../platform/audit.ts';
import { track, clientOf } from '../../telemetry/index.ts';
import { Status } from '../../../platform/http-status.ts';
import * as keyConfig from '../../config/service.ts';

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

/** Whether this is the key's first browser ever, remembering that the next one is not; a failed write only means a second `first`. */
function firstBrowser(key: string) {
  if (keyConfig.get(key).first_browser_at) return false;
  void keyConfig.set(key, { first_browser_at: new Date().toISOString() }).catch(() => {});
  return true;
}

/** Audits the start and answers 201 with `body`. */
export function started({ req, res, key, persona }: Start, body) {
  // `reused` keeps the trail honest: nothing was started, one was borrowed.
  const meta = { provider: body.provider, persona: persona.id, ...(body.reused ? { reused: true } : {}) };
  audit({ action: 'browser.start', actorKey: key, targetType: 'browser', targetId: body.id, meta, req });
  track.browserStarted(key, startedProps(req, key, persona, body));
  res.status(Status.CREATED).json(body);
}

/** What a browser_started event says about this start. */
const startedProps = (req, key: string, persona, body) => ({
  provider: String(body.provider),
  persona: !persona.isDefault,
  via: clientOf(req.headers),
  first: !body.reused && firstBrowser(key),
});

/** The body for a browser that is still booting and will dial in. */
export const starting = (start: Start, id: string, extra: object) => ({
  id,
  provider: start.wanted,
  persona: start.persona.id,
  status: 'starting',
  ...extra,
});
