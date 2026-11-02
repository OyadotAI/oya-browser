/**
 * What the save card says once a save answers: whether a rename happened, was
 * refused, or found the old playbook gone. "Renamed" is said only when the
 * main process reports the server confirmed it, so a person is never told a
 * playbook moved when it did not.
 */
import type { SaveResult } from '../../../../shared/ipc.ts';
import { CONFLICT, SAVE_SAY, SAY } from './constants.ts';

/** The message for a save that worked; `copyOf` names the playbook a Save as copy left alone. */
export function savedMessage(result: SaveResult, name: string, copyOf?: string): string {
  const from = result.renamed?.from;
  if (from && result.renamed?.missing) return SAVE_SAY.missing(from, name);
  if (from) return SAVE_SAY.renamed(from, name);
  return copyOf && copyOf !== name ? SAVE_SAY.copy(copyOf, name) : SAVE_SAY.saved(name);
}

/** The message for a save that failed: a taken name, a refused rename, a rename whose save failed, or the server's words. */
export function saveFailure(result: SaveResult | undefined, name: string): string {
  const error = result?.error || SAY.notConfirmed;
  const refused = result?.renameFailed;
  if (refused?.status === CONFLICT) return SAVE_SAY.taken(error);
  if (refused?.timedOut) return SAVE_SAY.renameUnknown(refused.from);
  if (refused) return SAVE_SAY.renameFailed(refused.from, error);
  const renamed = result?.renamed;
  return renamed && !renamed.missing ? SAVE_SAY.notSaved(renamed.from, name, error) : error;
}
