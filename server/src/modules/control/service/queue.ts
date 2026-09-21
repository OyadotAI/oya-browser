/**
 * The admission queue: fail queued sessions that expired or are no longer
 * admissible, then promote the next one that fits.
 */
import { openText } from '../../../platform/secrets.ts';
import { instanceId, live, stamp } from './model.ts';
import { atCapacity, projectsOf, spentUsd } from './capacity.ts';
import { MINUTES_PER_HOUR, PROVISIONING_LEASE_MS } from './constants.ts';

/** Priority order: lower runs first. */
const RANK = { high: 0, normal: 1, low: 2 };

/** Fail a queued session with `code`, announcing `reason`. */
function failQueued(tx, x, code, reason) {
  x.state = 'failed';
  x.errorCode = code;
  tx.emit(x.project, 'session.failed', x.id, { reason });
}

/** Fail a queued session whose deadline passed, or that the project's current settings no longer admit. */
function expire(tx, x, p) {
  if (x.deadline <= stamp()) failQueued(tx, x, 'queue_timeout', 'queue_timeout');
  else if (x.provider !== 'oya-selfhosted' && (p.settings.budgetUsd != null || Object.keys(p.settings.policy).length))
    failQueued(tx, x, 'unsupported_policy', 'Managed runtime required by current project settings');
}

/** Whether a queued session fits now: its budget has a rate, the project's budget has room, and a slot is free. */
function fits(x, p, sessions) {
  const rate = p.settings.rates[x.provider];
  if (x.budgetUsd != null && rate === undefined) return false;
  const budget = p.settings.budgetUsd;
  if (budget != null && (rate === undefined || spentUsd(sessions, p) + rate / MINUTES_PER_HOUR > budget)) return false;
  return !atCapacity(sessions, p, { maxConcurrent: x.hostCap, persona: x.persona, personaLimit: x.personaLimit });
}

/** Still-queued sessions, highest priority first, FIFO within one. */
const byPriority = (queued) =>
  queued
    .filter((x) => x.state === 'queued')
    .sort((a, b) => RANK[a.priority] - RANK[b.priority] || a.createdAt - b.createdAt);

/** A project's live sessions, read once per claim and cached in `active`. */
async function activeSessions(tx, active, id) {
  if (!active.has(id)) active.set(id, await tx.list('session', { project: id, states: live }));
  return active.get(id);
}

/** Still-queued sessions that fit now: highest priority first, FIFO within one. */
async function eligible(tx, queued, projects) {
  const active = new Map(),
    fitting = [];
  for (const x of byPriority(queued)) {
    const p = projects.get(x.project);
    if (fits(x, p, await activeSessions(tx, active, p.id))) fitting.push(x);
  }
  return fitting;
}

/** Prefer another project at the highest eligible priority, preserving FIFO per project. */
async function pickFairly(tx, fitting) {
  const first = fitting[0];
  const queue = await tx.get('meta', 'queue');
  return fitting.find((x) => x.priority === first.priority && x.project !== queue?.lastProject) || first;
}

/** Place a promoted session on this replica under a new fence. */
function lease(x) {
  Object.assign(x, {
    state: 'provisioning',
    provisioningActive: true,
    instance: instanceId,
    leaseUntil: stamp() + PROVISIONING_LEASE_MS,
    fence: x.fence + 1,
    meteredAt: stamp(),
  });
}

/** Apply the project's current runtime requirement, rate and policy to a promoted session. */
function price(x, p) {
  x.runtimeRequired ||= p.settings.budgetUsd != null || Object.keys(p.settings.policy).length > 0;
  x.rateUsdHour = p.settings.rates[x.provider] ?? null;
  x.reservedCostUsd = x.rateUsdHour === null ? 0 : x.rateUsdHour / MINUTES_PER_HOUR;
  x.policies = [...x.policies, p.settings.policy].filter((policy) => Object.keys(policy).length);
}

/** Start provisioning `x`; returns it with its project key and unsealed request. */
async function promote(tx, x, p) {
  await tx.lock('project', p.id);
  lease(x);
  price(x, p);
  tx.put('meta', 'queue', { id: 'queue', lastProject: x.project });
  tx.emit(x.project, 'session.provisioning', x.id);
  const key = openText(`control:${x.project}`, p.key);
  return { session: x, key, request: openText(`queue:${x.id}`, x.queuedRequest) };
}

/** Every queued session, or null while the fleet drains. */
async function queuedUnlessDraining(tx) {
  if ((await tx.get('meta', 'draining'))?.value) return null;
  return tx.list('session', { states: ['queued'] });
}

/** The claim transaction. */
async function claimNext(tx) {
  const queued = await queuedUnlessDraining(tx);
  if (!queued?.length) return null;
  const projects = await projectsOf(tx, queued);
  for (const x of queued) expire(tx, x, projects.get(x.project));
  const fitting = await eligible(tx, queued, projects);
  if (!fitting[0]) return null;
  const x = await pickFairly(tx, fitting);
  return promote(tx, x, projects.get(x.project));
}

/**
 * Fail expired or no-longer-admissible queued sessions, then promote the next
 * one that fits: highest priority first, rotating between projects, FIFO within one.
 * Returns it with its project key and unsealed request, or null.
 */
export function claimQueued(store) {
  return store.transact(claimNext);
}
