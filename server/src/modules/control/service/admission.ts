/**
 * Admission: whether a browser request may start before any provider is
 * called. Idempotency replay, drain, capacity (queueing when allowed), the
 * hourly cloud quota, governance and budgets, in that order.
 */
import { randomUUID } from 'node:crypto';
import { sealText } from '../../../platform/secrets.ts';
import { Status } from '../../../platform/http-status.ts';
import { capacityReached, fault, hash, instanceId, live, projectId, stamp } from './model.ts';
import { validatePolicy } from './policy.ts';
import { atCapacity, spentUsd } from './capacity.ts';
import { ensure } from './projects.ts';
import {
  DEFAULT_MAX_CONCURRENT,
  HOUR_MS,
  IDEMPOTENCY_WINDOW_MS,
  MAX_IDEMPOTENCY_KEY,
  MAX_QUEUE_MS,
  MAX_QUEUED,
  MINUTES_PER_HOUR,
  PROVISIONING_LEASE_MS,
} from './constants.ts';

/** reserve()'s option defaults, applied where the caller left a field undefined. */
const DEFAULTS = {
  persona: null,
  personaLimit: null,
  maxConcurrent: DEFAULT_MAX_CONCURRENT,
  hourlyLimit: 0,
  request: {},
  managed: false,
  cleanup: null,
  inheritedPolicies: [],
};

/** Refusal for a runtime that cannot enforce budgets. */
const unsupportedBudget = (message) => fault('unsupported_budget', message, Status.UNPROCESSABLE);

/** JSON with sorted object keys, so equal requests hash equally. */
const stable = (value) =>
  JSON.stringify(value, (_, v) =>
    v && typeof v === 'object' && !Array.isArray(v)
      ? Object.fromEntries(Object.entries(v).sort(([a], [b]) => a.localeCompare(b)))
      : v,
  );

/** The caller's options with defaults filled in, as destructuring defaults would. */
function withDefaults(options) {
  const o = { ...options, id: options.id === undefined ? randomUUID() : options.id };
  for (const [k, v] of Object.entries(DEFAULTS)) if (o[k] === undefined) o[k] = v;
  return o;
}

/** Reject a malformed Idempotency-Key or request policy. */
function validateKeyAndPolicy(o) {
  if (o.idempotencyKey && (typeof o.idempotencyKey !== 'string' || o.idempotencyKey.length > MAX_IDEMPOTENCY_KEY))
    throw fault('invalid_idempotency_key', 'Idempotency-Key must contain at most 200 characters', Status.BAD_REQUEST);
  if (o.request.policy) validatePolicy(o.request.policy);
}

/** Reject a malformed request before any storage is touched; fills in `queueMs`. */
function validateRequest(o) {
  validateKeyAndPolicy(o);
  o.queueMs = o.request.queueMs ?? 0;
  if (!Number.isInteger(o.queueMs) || o.queueMs < 0 || o.queueMs > MAX_QUEUE_MS)
    throw fault('invalid_queue', 'queueMs must be 0–300000', Status.BAD_REQUEST);
  const budget = o.request.budgetUsd;
  if (budget !== undefined && (!Number.isFinite(budget) || budget <= 0))
    throw fault('invalid_budget', 'Session budget must be positive USD', Status.BAD_REQUEST);
}

/** Everything admission reads, in one round trip. */
const admissionQueries = (pid, id, idempotency) => [
  { kind: 'project', id: pid },
  { kind: 'meta', id: 'draining' },
  { kind: 'session', id },
  { kind: 'session', project: pid, states: live },
  ...(idempotency ? [{ kind: 'idempotency', id: idempotency }] : []),
];

/**
 * Queue behind this replica's other admissions to the project, then read everything in one round trip:
 * the shorter the gap between reading the project and committing, the rarer conflicts with other replicas.
 */
async function prefetch(tx, key, o) {
  const pid = projectId(key),
    idempotency = o.idempotencyKey && `${pid}:${hash(o.idempotencyKey)}`;
  await tx.acquire('project', pid);
  await tx.prefetch(admissionQueries(pid, o.id, idempotency));
  return idempotency;
}

/** The session an earlier request with this Idempotency-Key created, if it is still recent. */
async function replayed(tx, idempotency, requestHash) {
  const prior = idempotency && (await tx.get('idempotency', idempotency));
  if (!prior || !(prior.createdAt > stamp() - IDEMPOTENCY_WINDOW_MS)) return null;
  if (prior.requestHash !== requestHash)
    throw fault('idempotency_conflict', 'Idempotency-Key was used for a different request');
  const session = await tx.get('session', prior.sessionId);
  return session ? { ...session, replay: true } : null;
}

/** Refuse while draining or for a taken id; then lock the project and read its live sessions. */
async function liveSessions(tx, p, id) {
  if ((await tx.get('meta', 'draining'))?.value)
    throw fault('draining', 'Control plane is draining', Status.UNAVAILABLE);
  if (await tx.get('session', id)) throw fault('session_exists', 'Session ID already exists');
  await tx.lock('project', p.id);
  return tx.list('session', { project: p.id, states: live });
}

/** Whether the request must queue; refuses when it may not wait or the queue is full. */
function mustQueue(sessions, p, o) {
  const full = atCapacity(sessions, p, o);
  if (full && !o.queueMs) throw capacityReached();
  if (full && sessions.filter((x) => x.state === 'queued').length >= MAX_QUEUED)
    throw fault('queue_full', 'Project queue is full', Status.TOO_MANY_REQUESTS);
  return full;
}

/** The project's cloud starts in the last hour; refuses past the hourly quota. */
function recentCloudStarts(p, o) {
  const recentCloud = (p.recentCloud || []).filter((t) => t >= stamp() - HOUR_MS);
  if (o.hourlyLimit && o.provider === 'oya-cloud' && recentCloud.length >= o.hourlyLimit)
    throw fault('hourly_quota', 'Sandbox hourly quota reached', Status.TOO_MANY_REQUESTS);
  return recentCloud;
}

/** Whether any governance applies: inherited, the project's, the request's own, or asked for. */
const governed = (p, o) =>
  o.inheritedPolicies.length || Object.keys(p.settings.policy).length || o.request.policy || o.request.governed;

/** Governance and project budgets are only enforced by the managed self-hosted runtime. */
function requireRuntime(p, o) {
  // A configured policy must never degrade silently on an unverified runtime.
  if (governed(p, o) && o.provider !== 'oya-selfhosted')
    throw fault(
      'runtime_not_verified',
      'Strict governance requires the managed self-hosted Docker runtime',
      Status.UNPROCESSABLE,
    );
  if ((p.settings.budgetUsd !== null || o.request.budgetUsd !== undefined) && o.provider !== 'oya-selfhosted')
    throw unsupportedBudget('Budgets require the managed self-hosted Docker runtime');
}

/** Refuse a budget the project cannot honour: budgets need a managed browser, and spend within the budget. */
function checkBudgets(sessions, p, o, rate, reserveUsd) {
  if (o.request.budgetUsd !== undefined && (!o.managed || rate === undefined))
    throw unsupportedBudget('Session budgets require a managed browser and rate card');
  if (p.settings.budgetUsd !== null && !o.managed) throw unsupportedBudget('Hard budgets require a managed browser');
  checkProjectBudget(sessions, p, rate, reserveUsd);
}

/** Refuse a session the project's budget cannot cover, or one without a rate to price it. */
function checkProjectBudget(sessions, p, rate, reserveUsd) {
  const budget = p.settings.budgetUsd;
  if (budget === null || (rate !== undefined && !(spentUsd(sessions, p) + reserveUsd > budget))) return;
  const message = rate === undefined ? 'A rate card is required for budgeted sessions' : 'Project budget exhausted';
  throw fault('budget_admission', message, Status.TOO_MANY_REQUESTS);
}

/** Every admission rule after capacity; returns the provider's rate and the cost reserved up front. */
function admit(sessions, p, o, full) {
  const recentCloud = recentCloudStarts(p, o);
  requireRuntime(p, o);
  const rate = p.settings.rates[o.provider];
  const reserveUsd = !full && rate !== undefined ? rate / MINUTES_PER_HOUR : 0;
  checkBudgets(sessions, p, o, rate, reserveUsd);
  if (o.provider === 'oya-cloud') p.recentCloud = [...recentCloud, stamp()];
  return { rate, reserveUsd };
}

/** Whether the session must run on the verified runtime: any governance or budget applies. */
const runtimeRequired = (p, o) =>
  !!(governed(p, o) || p.settings.budgetUsd !== null || o.request.budgetUsd !== undefined);

/** Who and what the new session is. */
function identity(p, o) {
  const replacementOf = o.request.replacementOf || null;
  const { id, provider, persona, managed } = o;
  return { id, project: p.id, runtimeRequired: runtimeRequired(p, o), provider, persona, managed, replacementOf };
}

/** Its policies, starting state and reserved cost. */
function standing(p, o, full, reserveUsd) {
  return {
    policies: [...o.inheritedPolicies, p.settings.policy, o.request.policy || {}].filter((x) => Object.keys(x).length),
    state: full ? 'queued' : 'provisioning',
    provisioningActive: !full,
    reservedCostUsd: reserveUsd,
    budgetUsd: o.request.budgetUsd ?? null,
  };
}

/** Its lease on this replica. */
function placement() {
  return {
    createdAt: stamp(),
    updatedAt: stamp(),
    instance: instanceId,
    fence: 1,
    leaseUntil: stamp() + PROVISIONING_LEASE_MS,
    control: { mode: 'agent' },
  };
}

/** Its request fingerprint, cleanup descriptor and rate. */
function bookkeeping(o, requestHash, rate) {
  const idempotencyKey = o.idempotencyKey || null;
  return { requestHash, idempotencyKey, cleanup: o.cleanup, rateUsdHour: rate ?? null, costUsd: 0 };
}

/** The request a queued session is started with later, sealed. */
function sealedRequest(o) {
  const { request, provider, persona } = o;
  return sealText(`queue:${o.id}`, { ...request, provider, persona, ...(persona ? { profile: persona } : {}) });
}

/** What a queued session waits with: its deadline, priority, sealed request and caps. */
function queueFields(o) {
  return {
    deadline: stamp() + o.queueMs,
    priority: ['low', 'normal', 'high'].includes(o.request.priority) ? o.request.priority : 'normal',
    queuedRequest: sealedRequest(o),
    hostCap: o.maxConcurrent,
    personaLimit: o.personaLimit,
  };
}

/** Remember which session an Idempotency-Key created. */
function recordIdempotency(tx, idempotency, p, id, requestHash) {
  if (!idempotency) return;
  tx.put('idempotency', idempotency, {
    id: idempotency,
    project: p.id,
    sessionId: id,
    requestHash,
    createdAt: stamp(),
  });
}

/** The admitted session's row; a queued one also carries what it waits with. */
function newSession(p, o, a) {
  const session = {
    ...identity(p, o),
    ...standing(p, o, a.full, a.reserveUsd),
    ...placement(),
    ...bookkeeping(o, a.requestHash, a.rate),
  };
  return a.full ? Object.assign(session, queueFields(o)) : session;
}

/** Store the admitted session and announce it. */
function recordSession(tx, p, o, a) {
  const session = newSession(p, o, a);
  tx.put('session', o.id, session);
  recordIdempotency(tx, a.idempotency, p, o.id, a.requestHash);
  tx.emit(p.id, `session.${session.state}`, o.id);
  return session;
}

/** The admission transaction: replay, or check every rule and record the session. */
async function admitInTx(tx, key, o, requestHash) {
  const idempotency = await prefetch(tx, key, o);
  const p = await ensure(tx, key);
  const replay = await replayed(tx, idempotency, requestHash);
  if (replay) return replay;
  const sessions = await liveSessions(tx, p, o.id);
  const full = mustQueue(sessions, p, o);
  return recordSession(tx, p, o, { full, requestHash, idempotency, ...admit(sessions, p, o, full) });
}

/**
 * Admit a browser request before any provider is called: idempotency replay,
 * drain, capacity (queueing when `queueMs` allows), hourly cloud quota,
 * governance and budget checks. Returns the session, `provisioning` or `queued`.
 */
export async function reserve(store, key, options: any = {}) {
  const o = withDefaults(options);
  validateRequest(o);
  const requestHash = hash(stable(o.request));
  return store.transact((tx) => admitInTx(tx, key, o, requestHash));
}
