/** Metering: bill a session's last unmetered interval when it stops holding capacity. */
import { holdsSlot, stamp } from './model.ts';
import { projectsOf } from './capacity.ts';
import { HOUR_MS } from './constants.ts';

/** Sessions this transaction moved out of holding a slot that have a rate to bill. */
const endedSessions = (tx) =>
  tx
    .changes('session')
    .filter(({ before, after }) => before && holdsSlot(before) && !holdsSlot(after) && after.rateUsdHour != null)
    .map(({ after }) => after);

/** Charge a session for the time since it was last metered, and its project with it. */
function meter(x, now, projects) {
  const cost = (Math.max(0, now - (x.meteredAt || x.createdAt)) / HOUR_MS) * x.rateUsdHour;
  x.costUsd += cost;
  x.meteredAt = now;
  if (projects.has(x.project)) projects.get(x.project).costUsd = (projects.get(x.project).costUsd || 0) + cost;
}

/** Bill a session's last unmetered interval when it stops holding capacity, whichever code path stopped it. */
export async function settleCosts(tx) {
  const ended = endedSessions(tx);
  if (!ended.length) return;
  const now = stamp(),
    projects = await projectsOf(tx, ended);
  for (const x of ended) meter(x, now, projects);
}
