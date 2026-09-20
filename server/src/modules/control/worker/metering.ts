/**
 * Cost metering: accrue estimated spend into sessions and their project, then
 * enforce session and project budgets.
 */
import { holdsSlot, live } from '../service.ts';
import { BUDGET_THRESHOLDS, HOUR_MS, METER_MS } from './constants.ts';

/** Only projects with rated sessions accrue cost; budgets require a rate card, so this also covers every budget. */
export async function meterRated(service, sessions) {
  for (const project of new Set(sessions.filter((x) => x.rateUsdHour != null && holdsSlot(x)).map((x) => x.project)))
    await meter(service, project);
}

/** Accrue estimated cost into the session and its project, then enforce session and project budgets. */
async function meter(service, id) {
  await service.store.transact((tx) => meterProject(tx, id));
}

/** The metering transaction for one project. */
async function meterProject(tx, id) {
  const now = Date.now(),
    p = await tx.get('project', id);
  if (!p) return;
  const sessions = await tx.list('session', { project: id, states: live });
  for (const x of sessions) accrue(x, p, now);
  const stop = new Set(sessions.filter((x) => overBudget(x)));
  if (p.settings.budgetUsd != null) enforceProjectBudget(tx, id, p, sessions, stop);
  for (const x of stop) markForCleanup(tx, id, x);
}

/** Adds the cost since the session was last metered, at least METER_MS apart. */
function accrue(x, p, now) {
  const since = x.meteredAt || x.createdAt;
  if (!holdsSlot(x) || x.rateUsdHour == null || now - since < METER_MS) return;
  const cost = ((now - since) / HOUR_MS) * x.rateUsdHour;
  x.costUsd += cost;
  p.costUsd = (p.costUsd || 0) + cost;
  x.meteredAt = now;
}

/** Whether a session's spend plus reservation has reached its own budget. */
const overBudget = (x) => x.budgetUsd != null && holdsSlot(x) && x.costUsd + (x.reservedCostUsd || 0) >= x.budgetUsd;

/** Raises each threshold alert once, and stops managed sessions once the project budget is spent. */
function enforceProjectBudget(tx, id, p, sessions, stop) {
  const budget = p.settings.budgetUsd,
    spent = p.costUsd || 0;
  for (const threshold of BUDGET_THRESHOLDS) alertOnce(tx, id, p, threshold, spent);
  // Queued work has no resource yet; it waits for budget or its deadline instead of entering cleanup.
  if (spent + sessions.reduce((n, x) => n + (x.reservedCostUsd || 0), 0) >= budget)
    for (const x of sessions) if (x.managed && holdsSlot(x)) stop.add(x);
}

/** Emits budget.threshold the first time spend crosses this fraction of this budget. */
function alertOnce(tx, id, p, threshold, spent) {
  const budget = p.settings.budgetUsd,
    alert = `${budget}:${threshold}`;
  if (!(spent >= budget * threshold) || p.alerts?.[alert]) return;
  (p.alerts ||= {})[alert] = true;
  tx.emit(id, 'budget.threshold', null, { threshold, estimatedUsd: spent });
}

/** Sends a session to cleanup for exceeding a budget. */
function markForCleanup(tx, id, x) {
  if (x.state === 'cleanup_pending') return;
  x.state = 'cleanup_pending';
  tx.emit(id, 'session.cleanup_pending', x.id, { reason: 'budget' });
}
