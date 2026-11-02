/** How much of a project is in use: browser and persona slots, and money spent or reserved. */
import { holdsSlot, terminal } from './model.ts';

/** The tighter of the project's and the caller's concurrency caps, or whichever is set. */
const capOf = (p, maxConcurrent) =>
  p.settings.maxConcurrent && maxConcurrent
    ? Math.min(p.settings.maxConcurrent, maxConcurrent)
    : (p.settings.maxConcurrent ?? maxConcurrent);

/** `sessions` are the project's live sessions; callers hold the project lock so the count cannot change underneath them. */
export function atCapacity(sessions, p, { maxConcurrent, persona, personaLimit }, except = null) {
  const active = sessions.filter((x) => x.id !== except && holdsSlot(x));
  const cap = capOf(p, maxConcurrent);
  return !!(
    (cap && active.length >= cap) ||
    (persona && personaLimit && active.filter((x) => x.persona === persona).length >= personaLimit)
  );
}

/** One persona a start may run as, with its cap (null for none). */
interface Choice {
  /** The persona's id. */
  persona: string;
  /** Its concurrency cap. */
  personaLimit: number | null;
}

/**
 * The persona a group start runs as: the one with the fewest slot-holding
 * sessions among those under their cap, else the fewest overall, so a start that
 * will queue waits on the one with the fewest running and waiting. Counted from `sessions` under the
 * project lock, never from what a replica has registered, so a burst of starts
 * spreads instead of all ranking the same member first. Ties keep `choices`'
 * order: the one used longest ago.
 */
export function leastLoaded(sessions, choices: Choice[]): Choice {
  const count = (list, c: Choice) => list.filter((x) => x.persona === c.persona).length;
  const [holding, waiting] = [sessions.filter(holdsSlot), sessions.filter((x) => x.state === 'queued')];
  const full = (c: Choice) => c.personaLimit !== null && count(holding, c) >= c.personaLimit;
  const load = (c: Choice) => count(holding, c) + count(waiting, c);
  return [...choices].sort((a, b) => Number(full(a)) - Number(full(b)) || load(a) - load(b))[0];
}

/** Metered spend (accumulated on the project, so pruning sessions loses nothing) plus outstanding reservations. */
export const spentUsd = (sessions, p) =>
  sessions.reduce((n, x) => n + (terminal.has(x.state) ? 0 : x.reservedCostUsd || 0), p.costUsd || 0);

/** The sessions' projects by id, loaded in one round trip. */
export async function projectsOf(tx, sessions) {
  const ids = sessions.map((x) => x.project);
  return new Map<string, any>((await tx.getMany('project', ids)).map((p) => [p.id, p]));
}
