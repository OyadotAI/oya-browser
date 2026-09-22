/**
 * Element ids across one turn's tool calls.
 *
 * The model reads one element list and may ask for several actions at once (fill
 * three fields, then submit). Each click or type re-analyzes the page, which numbers
 * its elements afresh, so a later call's id can name a different element by the
 * time it runs: the second field typed into the first. A later call's id is read in
 * the list the model saw and found again in the current one by the element's stable
 * handles, the same matching a playbook replay uses; one that is gone is refused.
 */
import { lastRun } from './recorder.ts';
import { matchElement } from '../playbooks/match.ts';

/** What a call is told when its element is gone from the page an earlier call left. */
export const ELEMENT_MOVED =
  'Error: Not run. An earlier call in this turn changed the page and this element is no longer on it. Use an id from the latest element list.';

/** The element list the model saw when it asked for this turn's calls. */
export type Batch = {
  /** That list, as the recorder held it. */
  elements: any[];
};

/** The page's current element list. */
const current = (browserId: string) => lastRun(browserId)?.elements || [];

/** Remembers the element list this turn's ids refer to. */
export const batchOf = (browserId: string): Batch => ({ elements: current(browserId) });

/**
 * The id `elementId` from the turn's list has in the page's current list: the same
 * id while nothing re-analyzed the page, the matching element's id after, or null
 * when no element there is the one the model meant.
 */
export function currentId(browserId: string, batch: Batch, elementId: unknown) {
  const now = current(browserId);
  if (elementId == null || now === batch.elements) return elementId;
  const meant = batch.elements.find((e) => e.id === Number(elementId));
  return meant ? (matchElement(meant, now)?.id ?? null) : elementId;
}
