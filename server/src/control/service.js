import { createHash, randomUUID, randomBytes } from 'node:crypto';
import { controlStore } from './store.js';
import { sealText, openText } from '../secrets.js';

export const instanceId = process.env.OYA_INSTANCE_ID || randomUUID();
export const terminal = new Set(['stopped', 'failed']);
/** Every non-terminal state: the session may still own a resource, a slot, or a queue position. */
export const live = ['queued', 'provisioning', 'ready', 'disconnected', 'stopping', 'cleanup_pending', 'unknown_outcome'];
/** Providers with no resource of ours to delete: a failed or lost session is simply over. */
export const attachOnly = new Set(['cdp', 'oya-desktop', 'gateway']);
/** Capacity is held while a resource may exist. Disconnected sessions have no deletion descriptor and re-check capacity on reconnect. */
export const holdsSlot = x => !terminal.has(x.state) && x.state !== 'queued' && x.state !== 'disconnected';
export const hash = value => createHash('sha256').update(value).digest('hex');
export const projectId = key => `prj_${hash(key).slice(0, 24)}`;
export const fault = (code, message, status = 409) => Object.assign(new Error(message), { code, status });
const stamp = () => Date.now();
const DAY = 86400000;
async function ensure(tx, key) {
  const id = projectId(key);
  let p = await tx.get('project', id);
  if (!p) {
    p = tx.put('project', id, { id, name: `Project ${id.slice(-6)}`, createdAt: stamp(), legacyOwner: hash(key).slice(0, 16), key: sealText(`control:${id}`, key), costUsd: 0, settings: { recordingDays: 7, auditDays: 90, budgetUsd: null, maxConcurrent: null, rates: {}, policy: {} } });
    tx.emit(id, 'project.created');
  }
  if (p.deletedAt) throw fault('project_deleted', 'Project has been deleted', 410);
  return p;
}
/**
 * The API key a project was created with, by id — what background work uses when
 * it has a project and no caller. Null rather than throwing where the caller is a
 * worker or a redirect that can only drop the job, not report a 503 to anyone.
 */
export async function keyOfProject(id, service = control()) {
  const p = await service.store.get('project', id);
  if (!p?.key) return null;
  try { return service.projectKey(p); } catch { return null; }
}
const publicProject = ({ key, alerts, recentCloud, ...rest }) => rest;
const publicSession = ({ cleanup, response, requestHash, queuedRequest, egressHash, enrollmentHash, ...rest }) => rest;
const stable = value => JSON.stringify(value, (_, v) => v && typeof v === 'object' && !Array.isArray(v) ? Object.fromEntries(Object.entries(v).sort(([a], [b]) => a.localeCompare(b))) : v);
/** `sessions` are the project's live sessions; callers hold the project lock so the count cannot change underneath them. */
function atCapacity(sessions, p, { maxConcurrent, persona, personaLimit }, except = null) {
  const active = sessions.filter(x => x.id !== except && holdsSlot(x));
  const cap = p.settings.maxConcurrent && maxConcurrent ? Math.min(p.settings.maxConcurrent, maxConcurrent) : (p.settings.maxConcurrent ?? maxConcurrent);
  return !!((cap && active.length >= cap) || (persona && personaLimit && active.filter(x => x.persona === persona).length >= personaLimit));
}
/** Metered spend (accumulated on the project, so pruning sessions loses nothing) plus outstanding reservations. */
const spentUsd = (sessions, p) => sessions.reduce((n, x) => n + (terminal.has(x.state) ? 0 : x.reservedCostUsd || 0), p.costUsd || 0);

// Shaped like the hostname rule it replaces — deliberately no TLD anchor, which would
// reject `localhost`, IP literals and punycode TLDs that are valid today — plus `*`
// as a label character so mid-label rules like `*-aiplatform.googleapis.com` parse.
const HOST_RULE = /^(\*\.)?[a-z0-9*](?:[a-z0-9*.-]*[a-z0-9*])?$/;
// Each `*` compiles to an unbounded quantifier and the engine enumerates every
// partition on a failed match: on a 30-character host, 8 stars is 0.4s, 10 is 7.7s and
// 12 saturates around 150s. The policy check runs before any DNS, on the shared
// control plane, so one CONNECT would stall every tenant. Three stars against a
// 2000-character host is 0ms. The length cap is for cache memory, not backtracking —
// the pathological rules are only ~30 characters long.
const badHostRule = h => typeof h !== 'string' || h.length > 253
  || (h.match(/\*/g) || []).length > 3
  || !HOST_RULE.test(h);

export function validatePolicy(policy) {
  if (!policy || typeof policy !== 'object' || Array.isArray(policy) || Object.keys(policy).some(k => !['allowedHosts', 'humanHosts', 'region', 'redactRecording'].includes(k))) throw fault('invalid_policy', 'Unknown policy field', 400);
  for (const name of ['allowedHosts', 'humanHosts']) {
    if (name in policy && (!Array.isArray(policy[name]) || !policy[name].length || policy[name].length > 100 || policy[name].some(badHostRule))) throw fault('invalid_policy', `${name} must contain up to 100 lowercase hostname, *.domain or mid-label wildcard rules`, 400);
  }
  if ('region' in policy && (typeof policy.region !== 'string' || !/^[a-z0-9_-]{1,40}$/.test(policy.region))) throw fault('invalid_policy', 'Invalid region', 400);
  if ('redactRecording' in policy && typeof policy.redactRecording !== 'boolean') throw fault('invalid_policy', 'redactRecording must be boolean', 400);
}
function validateSettings(changes) {
  const allowed = ['recordingDays', 'auditDays', 'budgetUsd', 'maxConcurrent', 'rates', 'policy'];
  if (!changes || typeof changes !== 'object' || Object.keys(changes).some(k => !allowed.includes(k))) throw fault('invalid_settings', 'Unknown project setting', 400);
  for (const k of ['recordingDays', 'auditDays']) if (k in changes && (!Number.isInteger(changes[k]) || changes[k] < 1 || changes[k] > 3650)) throw fault('invalid_retention', `${k} must be 1–3650 days`, 400);
  for (const k of ['budgetUsd', 'maxConcurrent']) if (k in changes && changes[k] !== null && (!Number.isFinite(changes[k]) || changes[k] <= 0 || (k === 'maxConcurrent' && !Number.isInteger(changes[k])))) throw fault('invalid_limit', `${k} must be positive or null`, 400);
  if ('rates' in changes && (!changes.rates || typeof changes.rates !== 'object' || Array.isArray(changes.rates) || Object.values(changes.rates).some(v => !Number.isFinite(v) || v < 0))) throw fault('invalid_rates', 'Rates must be USD per browser hour', 400);
  if ('policy' in changes) validatePolicy(changes.policy);
}
/** Bill a session's last unmetered interval when it stops holding capacity, whichever code path stopped it. */
async function settleCosts(tx) {
  const ended = tx.changes('session').filter(({ before, after }) => before && holdsSlot(before) && !holdsSlot(after) && after.rateUsdHour != null);
  if (!ended.length) return;
  const now = stamp(), projects = new Map((await tx.getMany('project', ended.map(c => c.after.project))).map(p => [p.id, p]));
  for (const { after: x } of ended) {
    const cost = Math.max(0, now - (x.meteredAt || x.createdAt)) / 3600000 * x.rateUsdHour;
    x.costUsd += cost; x.meteredAt = now;
    if (projects.has(x.project)) projects.get(x.project).costUsd = (projects.get(x.project).costUsd || 0) + cost;
  }
}
/** Stop a live session. Force is the operator's reconciliation: with no deletion descriptor and no creation in flight, they assert the resource is gone. */
function stopSession(tx, x, { force = false, reason = 'cancelled' } = {}) {
  const reconcile = force && x.state !== 'queued' && !x.cleanup && !(x.provisioningActive && x.leaseUntil > stamp());
  x.state = x.state === 'queued' || reconcile ? 'stopped' : 'cleanup_pending';
  tx.emit(x.project, `session.${x.state}`, x.id, { reason: reconcile ? 'reconciled' : reason });
}
async function ownSession(tx, key, id) {
  const x = await tx.get('session', id);
  if (!x || x.project !== projectId(key)) throw fault('not_found', 'Session not found', 404);
  return x;
}

export class ControlService {
  constructor(store = controlStore()) { this.store = store; store.beforeCommit = settleCosts; }
  async project(key) {
    const p = await this.store.get('project', projectId(key));
    if (p?.deletedAt) throw fault('project_deleted', 'Project has been deleted', 410);
    return publicProject(p || await this.store.transact(tx => ensure(tx, key)));
  }
  projectKey(project) {
    try { return openText(`control:${project.id}`, project.key); }
    catch { throw fault('project_key_unavailable', 'Project credentials could not be decrypted. Restore the original OYA_PROFILE_SECRET and OYA_PROFILE_SALT on every server, or add the original API key again to repair project access.', 503); }
  }
  async updateOwnedProject(userId, id, { name, remove = false, stopBrowsers = false } = {}) {
    return this.store.transact(async tx => {
      const p = await tx.get('project', id);
      if (!p || p.ownerUser !== userId || p.deletedAt) throw fault('not_found', 'Project not found', 404);
      if (remove) {
        const open = (await tx.list('session', { project: id })).filter(s => !terminal.has(s.state));
        if (open.length && !stopBrowsers) throw Object.assign(fault('project_active', `Stop ${open.length} browser${open.length === 1 ? '' : 's'} before deleting this project`), { active: open.length });
        // Deleting is the owner's final word, so a session nothing can reach (disconnected, unknown outcome) ends here.
        // Resources with a deletion descriptor keep cleaning up after the project is gone.
        for (const x of open) stopSession(tx, x, { force: true, reason: 'project_deleted' });
        p.deletedAt = stamp();
        for (const c of await tx.list('credential', { project: id })) c.revokedAt = stamp();
        for (const m of await tx.list('membership', { project: id })) await tx.delete('membership', m.id);
        tx.emit(id, 'project.deleted');
      } else {
        if (typeof name !== 'string' || !name.trim() || name.trim().length > 100) throw fault('invalid_name', 'Project name must be 1–100 characters', 400);
        p.name = name.trim();
        tx.emit(id, 'project.renamed');
      }
      return { ok: true };
    });
  }
  async read(key) {
    const project = await this.project(key), id = project.id;
    const [sessions, credentials, webhooks, deliveries, [draining]] = await this.store.load([
      { kind: 'session', project: id }, { kind: 'credential', project: id }, { kind: 'webhook', project: id },
      // Delivered notifications need no attention; the overview lists the rest.
      { kind: 'delivery', project: id, states: ['pending', 'failed', 'cancelled'] }, { kind: 'meta', id: 'draining' },
    ]);
    return {
      project, draining: !!draining?.body.value,
      sessions: sessions.map(r => publicSession(r.body)),
      events: await this.store.events({ project: id, latest: true, limit: 100 }),
      credentials: credentials.map(r => r.body).filter(c => c.role !== 'browser').map(({ digest, ...c }) => c),
      webhooks: webhooks.map(({ body: { secret, ...hook } }) => hook),
      deliveries: deliveries.map(r => r.body),
    };
  }
  async sessions(key, states) { return (await this.store.list('session', { project: projectId(key), ...(states ? { states } : {}) })).map(publicSession); }
  /** This project's session, or null. */
  async findSession(key, id) {
    const x = await this.store.get('session', id);
    return x?.project === projectId(key) ? publicSession(x) : null;
  }
  async session(key, id) {
    const x = await this.findSession(key, id);
    if (!x) throw fault('not_found', 'Session not found', 404);
    return x;
  }
  /** The retained event log, oldest first after the cursor. */
  events(key, { after = 0, limit = 500 } = {}) { return this.store.events({ project: projectId(key), after, limit }); }
  async settings(key, changes) {
    validateSettings(changes);
    return this.store.transact(async tx => {
      const p = await ensure(tx, key);
      p.settings = { ...p.settings, ...changes };
      tx.emit(p.id, 'project.settings.updated', null, { fields: Object.keys(changes) });
      return publicProject(p);
    });
  }
  async holdProvider(owner, name, capacity) {
    const id = randomUUID(), resource = `${owner || '@shared'}:${name}`;
    return this.store.transact(async tx => {
      await tx.lock('meta', `hold:${resource}`);
      if ((await tx.list('hold', { states: [resource] })).filter(h => h.expiresAt >= stamp()).length >= capacity) throw fault('provider_capacity', 'Provider capacity is reserved on another replica', 429);
      tx.put('hold', id, { id, resource, expiresAt: stamp() + 180000 });
      return id;
    });
  }
  async releaseProvider(id) { return this.store.transact(async tx => { if (await tx.get('hold', id)) await tx.delete('hold', id); }); }
  async reserve(key, { id = randomUUID(), provider, persona = null, personaLimit = null, maxConcurrent = 5000, hourlyLimit = 0, request = {}, idempotencyKey, managed = false, cleanup = null, inheritedPolicies = [] } = {}) {
    if (idempotencyKey && (typeof idempotencyKey !== 'string' || idempotencyKey.length > 200)) throw fault('invalid_idempotency_key', 'Idempotency-Key must contain at most 200 characters', 400);
    if (request.policy) validatePolicy(request.policy);
    const queueMs = request.queueMs ?? 0;
    if (!Number.isInteger(queueMs) || queueMs < 0 || queueMs > 300000) throw fault('invalid_queue', 'queueMs must be 0–300000', 400);
    if (request.budgetUsd !== undefined && (!Number.isFinite(request.budgetUsd) || request.budgetUsd <= 0)) throw fault('invalid_budget', 'Session budget must be positive USD', 400);
    const requestHash = hash(stable(request));
    return this.store.transact(async tx => {
      // Queue behind this replica's other admissions to the project, then read everything in one round trip:
      // the shorter the gap between reading the project and committing, the rarer conflicts with other replicas.
      const pid = projectId(key), idempotency = idempotencyKey && `${pid}:${hash(idempotencyKey)}`;
      await tx.acquire('project', pid);
      await tx.prefetch([{ kind: 'project', id: pid }, { kind: 'meta', id: 'draining' }, { kind: 'session', id }, { kind: 'session', project: pid, states: live }, ...(idempotency ? [{ kind: 'idempotency', id: idempotency }] : [])]);
      const p = await ensure(tx, key);
      const prior = idempotency && await tx.get('idempotency', idempotency);
      if (prior && prior.createdAt > stamp() - 7 * DAY) {
        if (prior.requestHash !== requestHash) throw fault('idempotency_conflict', 'Idempotency-Key was used for a different request');
        const session = await tx.get('session', prior.sessionId);
        if (session) return { ...session, replay: true };
      }
      if ((await tx.get('meta', 'draining'))?.value) throw fault('draining', 'Control plane is draining', 503);
      if (await tx.get('session', id)) throw fault('session_exists', 'Session ID already exists');
      await tx.lock('project', p.id);
      const sessions = await tx.list('session', { project: p.id, states: live });
      const full = atCapacity(sessions, p, { maxConcurrent, persona, personaLimit });
      if (full && !queueMs) throw fault('quota_exceeded', 'Browser or persona capacity reached', 429);
      if (full && sessions.filter(x => x.state === 'queued').length >= 1000) throw fault('queue_full', 'Project queue is full', 429);
      const recentCloud = (p.recentCloud || []).filter(t => t >= stamp() - 3600000);
      if (hourlyLimit && provider === 'oya-cloud' && recentCloud.length >= hourlyLimit) throw fault('hourly_quota', 'Sandbox hourly quota reached', 429);
      const policy = p.settings.policy;
      // A configured policy must never degrade silently on an unverified runtime.
      if ((inheritedPolicies.length || Object.keys(policy).length || request.policy || request.governed) && provider !== 'oya-selfhosted') throw fault('runtime_not_verified', 'Strict governance requires the managed self-hosted Docker runtime', 422);
      if ((p.settings.budgetUsd !== null || request.budgetUsd !== undefined) && provider !== 'oya-selfhosted') throw fault('unsupported_budget', 'Budgets require the managed self-hosted Docker runtime', 422);
      const rate = p.settings.rates[provider];
      if (request.budgetUsd !== undefined && (!managed || rate === undefined)) throw fault('unsupported_budget', 'Session budgets require a managed browser and rate card', 422);
      const reserveUsd = !full && rate !== undefined ? rate / 60 : 0;
      if (p.settings.budgetUsd !== null && !managed) throw fault('unsupported_budget', 'Hard budgets require a managed browser', 422);
      if (p.settings.budgetUsd !== null && (rate === undefined || spentUsd(sessions, p) + reserveUsd > p.settings.budgetUsd)) throw fault('budget_admission', rate === undefined ? 'A rate card is required for budgeted sessions' : 'Project budget exhausted', 429);
      if (provider === 'oya-cloud') p.recentCloud = [...recentCloud, stamp()];
      const session = { id, project: p.id, runtimeRequired: !!(inheritedPolicies.length || request.governed || Object.keys(policy).length || request.policy || p.settings.budgetUsd !== null || request.budgetUsd !== undefined), provider, persona, managed, replacementOf: request.replacementOf || null, policies: [...inheritedPolicies, policy, request.policy || {}].filter(x => Object.keys(x).length), state: full ? 'queued' : 'provisioning', provisioningActive: !full, reservedCostUsd: reserveUsd, budgetUsd: request.budgetUsd ?? null, createdAt: stamp(), updatedAt: stamp(), instance: instanceId, fence: 1, leaseUntil: stamp() + 180000, control: { mode: 'agent' }, requestHash, idempotencyKey: idempotencyKey || null, cleanup, rateUsdHour: rate ?? null, costUsd: 0 };
      if (full) Object.assign(session, { deadline: stamp() + queueMs, priority: ['low', 'normal', 'high'].includes(request.priority) ? request.priority : 'normal', queuedRequest: sealText(`queue:${id}`, { ...request, provider, persona, ...(persona ? { profile: persona } : {}) }), hostCap: maxConcurrent, personaLimit });
      tx.put('session', id, session);
      if (idempotency) tx.put('idempotency', idempotency, { id: idempotency, project: p.id, sessionId: id, requestHash, createdAt: stamp() });
      tx.emit(p.id, `session.${session.state}`, id);
      return session;
    });
  }
  async claimQueued() {
    return this.store.transact(async tx => {
      if ((await tx.get('meta', 'draining'))?.value) return null;
      const queued = await tx.list('session', { states: ['queued'] });
      if (!queued.length) return null;
      const projects = new Map((await tx.getMany('project', queued.map(x => x.project))).map(p => [p.id, p]));
      for (const x of queued) {
        const p = projects.get(x.project);
        if (x.deadline <= stamp()) { x.state = 'failed'; x.errorCode = 'queue_timeout'; tx.emit(x.project, 'session.failed', x.id, { reason: 'queue_timeout' }); }
        else if (x.provider !== 'oya-selfhosted' && (p.settings.budgetUsd != null || Object.keys(p.settings.policy).length)) {
          x.state = 'failed'; x.errorCode = 'unsupported_policy'; tx.emit(x.project, 'session.failed', x.id, { reason: 'Managed runtime required by current project settings' });
        }
      }
      const rank = { high: 0, normal: 1, low: 2 };
      const candidates = queued.filter(x => x.state === 'queued').sort((a, b) => rank[a.priority] - rank[b.priority] || a.createdAt - b.createdAt);
      const active = new Map(), eligible = [];
      for (const x of candidates) {
        const p = projects.get(x.project), rate = p.settings.rates[x.provider];
        if (!active.has(p.id)) active.set(p.id, await tx.list('session', { project: p.id, states: live }));
        if (x.budgetUsd != null && rate === undefined) continue;
        if (p.settings.budgetUsd != null && (rate === undefined || spentUsd(active.get(p.id), p) + rate / 60 > p.settings.budgetUsd)) continue;
        if (!atCapacity(active.get(p.id), p, { maxConcurrent: x.hostCap, persona: x.persona, personaLimit: x.personaLimit })) eligible.push(x);
      }
      const first = eligible[0];
      if (!first) return null;
      // Prefer another project at the highest eligible priority, preserving FIFO per project.
      const queue = await tx.get('meta', 'queue');
      const x = eligible.find(x => x.priority === first.priority && x.project !== queue?.lastProject) || first, p = projects.get(x.project);
      await tx.lock('project', p.id);
      Object.assign(x, { state: 'provisioning', provisioningActive: true, instance: instanceId, leaseUntil: stamp() + 180000, fence: x.fence + 1, meteredAt: stamp() });
      x.runtimeRequired ||= p.settings.budgetUsd != null || Object.keys(p.settings.policy).length > 0;
      x.rateUsdHour = p.settings.rates[x.provider] ?? null;
      x.reservedCostUsd = x.rateUsdHour === null ? 0 : x.rateUsdHour / 60;
      x.policies = [...x.policies, p.settings.policy].filter(policy => Object.keys(policy).length);
      tx.put('meta', 'queue', { id: 'queue', lastProject: x.project });
      tx.emit(x.project, 'session.provisioning', x.id);
      return { session: x, key: openText(`control:${x.project}`, p.key), request: openText(`queue:${x.id}`, x.queuedRequest) };
    });
  }
  async cancel(key, id, { force = false } = {}) {
    return this.store.transact(async tx => {
      const x = await ownSession(tx, key, id);
      if (terminal.has(x.state)) return publicSession(x);
      stopSession(tx, x, { force });
      return publicSession(x);
    });
  }
  async complete(key, id, status, body) {
    return this.store.transact(async tx => {
      const x = await ownSession(tx, key, id);
      x.response = { status, body };
      x.provisioningActive = false;
      if (['cleanup_pending', 'stopping', 'stopped'].includes(x.state)) return publicSession(x);
      const next = status < 400 ? (x.state === 'ready' || body.status !== 'starting' ? 'ready' : 'provisioning') : (x.cleanup ? 'cleanup_pending' : status >= 500 && !attachOnly.has(x.provider) ? 'unknown_outcome' : 'failed');
      if (x.state !== next) { x.state = next; tx.emit(x.project, `session.${next}`, id); }
      return publicSession(x);
    });
  }
  async assertProvisioning(key, id) {
    const x = await this.store.get('session', id);
    if (!x || x.project !== projectId(key) || !['provisioning', 'ready'].includes(x.state)) throw fault('creation_cancelled', 'Session creation was cancelled');
    if (x.instance !== instanceId || x.leaseUntil < stamp()) throw fault('stale_worker', 'Session provisioning lease expired');
  }
  async update(key, id, changes, { fence } = {}) {
    return this.store.transact(async tx => {
      const x = await ownSession(tx, key, id);
      if (fence !== undefined && x.fence !== fence) throw fault('stale_worker', 'Session ownership changed');
      if (terminal.has(x.state) && changes.state && changes.state !== x.state) throw fault('terminal_session', 'Session is already terminal');
      if (['stopping', 'cleanup_pending'].includes(x.state) && changes.state && !['stopping', 'cleanup_pending', 'stopped'].includes(changes.state)) throw fault('session_stopping', 'Session is stopping');
      const before = x.state;
      Object.assign(x, changes, { updatedAt: stamp() });
      if (before !== x.state) tx.emit(x.project, `session.${x.state}`, id);
      return publicSession(x);
    });
  }
  async adopt(key, { id, provider, persona = null, maxConcurrent, personaLimit } = {}) {
    // Reserve first if this is a browser connecting without a provisioning operation.
    if (!(await this.store.get('session', id))) {
      try { await this.reserve(key, { id, provider, persona, maxConcurrent, personaLimit }); }
      catch (e) { if (e.code !== 'session_exists') throw e; }
    }
    return this.store.transact(async tx => {
      const x = await ownSession(tx, key, id);
      if (terminal.has(x.state) || ['stopping', 'cleanup_pending'].includes(x.state)) throw fault('session_stopped', 'Session no longer accepts connections');
      // A provisioning browser enrolls wherever it lands; managed ones are bound by their enrollment token.
      if (x.instance !== instanceId && x.leaseUntil > stamp() && x.state !== 'provisioning') throw fault('session_owned', 'Session is connected to another replica');
      if (x.state === 'disconnected') {
        const p = await tx.get('project', x.project);
        await tx.lock('project', p.id);
        if (atCapacity(await tx.list('session', { project: p.id, states: live }), p, { maxConcurrent, persona, personaLimit }, id)) throw fault('quota_exceeded', 'Browser or persona capacity reached', 429);
        x.meteredAt = stamp();
      }
      Object.assign(x, { state: 'ready', instance: instanceId, leaseUntil: stamp() + 30000, fence: x.fence + 1, persona, provider: x.provider === 'legacy' ? provider : x.provider });
      tx.emit(x.project, 'session.ready', id);
      return publicSession(x);
    });
  }
  async drain(value) {
    return this.store.transact(async tx => {
      await tx.get('meta', 'draining');
      tx.put('meta', 'draining', { id: 'draining', value: !!value });
      return !!value;
    });
  }
  /** The project a key opens, without touching storage. */
  projectIdFor(key) { return projectId(key); }
  async ticket(key, sessionId, authToken = key) {
    const token = randomBytes(32).toString('base64url');
    await this.store.transact(async tx => { tx.put('ticket', hash(token), { project: projectId(key), sessionId, auth: sealText('connection-ticket', authToken), expiresAt: stamp() + 60000 }); });
    return token;
  }
  async redeem(token, sessionId) {
    return this.store.transact(async tx => {
      const t = await tx.get('ticket', hash(token));
      if (!t || t.expiresAt < stamp() || t.sessionId !== sessionId) throw fault('invalid_ticket', 'Invalid connection ticket', 401);
      await tx.delete('ticket', hash(token));
      return openText('connection-ticket', t.auth);
    });
  }
  async credential(key, { role = 'operator', label = 'Service account', expiresAt = null } = {}, memberUser = null) {
    if (!['viewer', 'operator', 'administrator'].includes(role)) throw fault('invalid_role', 'Unknown role', 400);
    if (expiresAt !== null && (!Number.isFinite(expiresAt) || expiresAt <= stamp())) throw fault('invalid_expiry', 'Expiry must be a future timestamp', 400);
    return this.#issue(key, { role, label, expiresAt, memberUser });
  }
  /** A managed browser's own credential: it can only register that session's browser, and it ends with the session. */
  enrollmentCredential(key, sessionId) { return this.#issue(key, { role: 'browser', label: 'Managed browser', expiresAt: null, memberUser: null, sessionId }); }
  /**
   * A shareable, expiring credential for one live browser — embed it in a link
   * and hand it to a viewer, or to another app via the SDK. `control` decides
   * whether the holder can only watch (viewer) or also take over and act
   * (operator). The credential is bound to this one session and confined to its
   * live-view, ticket, input and control endpoints by authMiddleware, so a share
   * link never becomes project-wide access. Revoke it like any credential.
   */
  async share(key, { id, control = false, expiresIn = 3600 } = {}) {
    const x = await this.findSession(key, id);
    if (!x || terminal.has(x.state)) throw fault('not_found', 'Ready session not found', 404);
    const seconds = Math.min(Math.max(Math.floor(Number(expiresIn) || 3600), 60), 30 * 86400);
    return this.#issue(key, {
      role: control ? 'operator' : 'viewer',
      label: control ? 'Shared browser (control)' : 'Shared browser (view)',
      expiresAt: stamp() + seconds * 1000, memberUser: null, sessionId: id,
    });
  }
  async #issue(key, { role, label, expiresAt, memberUser, sessionId = null }) {
    const token = `oya_${randomBytes(32).toString('base64url')}`, digest = hash(token), id = randomUUID();
    const credential = await this.store.transact(async tx => {
      const p = await ensure(tx, key);
      if (memberUser && (p.ownerUser === memberUser ? 'administrator' : (await tx.get('membership', `${p.id}:${memberUser}`))?.role) !== role) throw fault('membership_changed', 'Project membership changed', 403);
      const { digest: _, ...publicValue } = tx.put('credential', digest, { id, digest, project: p.id, role, memberUser, sessionId, label: String(label).slice(0, 100), expiresAt, createdAt: stamp(), revokedAt: null });
      tx.emit(p.id, 'credential.created', null, { id, role });
      return publicValue;
    });
    return { ...credential, token };
  }
  async authenticate(token) {
    const c = await this.store.get('credential', hash(token));
    if (!c) return null;
    const [[project], [membership], [session]] = await this.store.load([{ kind: 'project', id: c.project }, { kind: 'membership', id: `${c.project}:${c.memberUser}` }, { kind: 'session', id: String(c.sessionId) }]);
    if (!project || project.body.deletedAt) throw fault('project_deleted', 'Project has been deleted', 410);
    if (c.memberUser && (project.body.ownerUser === c.memberUser ? 'administrator' : membership?.body.role) !== c.role) throw fault('membership_removed', 'Project access was removed', 403);
    if (c.revokedAt || (c.expiresAt && c.expiresAt <= stamp())) throw fault('revoked_credential', 'Credential expired or revoked', 401);
    if (c.role === 'browser' && (!session || terminal.has(session.body.state))) throw fault('revoked_credential', 'Managed browser session has ended', 401);
    return { key: this.projectKey(project.body), role: c.role, project: c.project, credentialId: c.id, sessionId: c.sessionId };
  }
  async revoke(key, id) {
    return this.store.transact(async tx => {
      const c = (await tx.list('credential', { project: projectId(key) })).find(c => c.id === id);
      if (!c) throw fault('not_found', 'Credential not found', 404);
      c.revokedAt = stamp(); tx.emit(c.project, 'credential.revoked', null, { id }); return { ok: true };
    });
  }
  /**
   * `force` takes the browser from whoever is holding it. The hold exists so two
   * operators do not fight over one page, not to lock out the project that owns it:
   * a tab that closed without releasing, or a lease renewed by a forgotten dialog,
   * otherwise blocks its owner for five minutes with nothing they can do about it.
   */
  async takeover(key, id, action, holder, { force = false } = {}) {
    return this.store.transact(async tx => {
      const x = await ownSession(tx, key, id);
      const revision = Math.max(stamp(), (x.control.revision || 0) + 1);
      if (x.state !== 'ready') throw fault('not_ready', 'Session must be ready');
      if (x.control.takeover && x.control.expiresAt > stamp() && x.control.holder !== holder) throw fault('control_busy', 'Another operator is taking control');
      if (action === 'request') {
        if (x.control.mode === 'human' && x.control.expiresAt > stamp() && x.control.holder !== holder) throw fault('control_busy', 'Another operator has control');
        if (x.control.mode !== 'human' || x.control.expiresAt <= stamp()) x.control = { mode: 'paused', holder, takeover: true, expiresAt: stamp() + 10000 };
      } else if (action === 'renew') {
        if (x.control.mode !== 'human' || x.control.holder !== holder || x.control.expiresAt <= stamp()) throw fault('control_busy', 'Human control has expired or changed');
        x.control.expiresAt = stamp() + 300000;
      } else if (action === 'return') {
        if (x.control.mode === 'agent') return x.control;
        if (x.control.holder !== holder || !['human', 'paused'].includes(x.control.mode)) throw fault('control_busy', 'Only the current operator can return control');
        x.control = { mode: 'agent' };
      } else if (action === 'acquire') {
        if (x.inFlight > 0) throw fault('commands_pending', 'In-flight commands must settle before takeover');
        if (!force && x.control.mode === 'human' && x.control.expiresAt > stamp() && x.control.holder !== holder) throw fault('control_busy', 'Another operator has control');
        x.control = { mode: 'human', holder, expiresAt: stamp() + 300000 };
      } else if (action === 'release') {
        if (x.control.mode === 'human' && x.control.holder !== holder) throw fault('control_busy', 'Another operator has control');
        x.control = { mode: 'paused' };
      } else if (action === 'resume') {
        if (x.control.mode === 'human' && x.control.expiresAt > stamp()) throw fault('control_busy', 'Human control must be released first');
        x.control = { mode: 'agent' };
      } else throw fault('invalid_action', 'Use acquire, release, or resume', 400);
      x.control.revision = revision;
      tx.emit(x.project, `control.${x.control.mode}`, id);
      return x.control;
    });
  }
  async beginCommand(id, holder = null) {
    const fence = await this.store.beginCommand(id, holder, instanceId);
    let ended = false;
    return async () => { if (ended) return; ended = true; await this.store.finishCommand(id, fence); };
  }
  /**
   * The project's one customer endpoint, Stripe-style: a derived id so saving again
   * edits it rather than stacking a second hook. The secret survives edits and is
   * returned only when minted — on first save or on `roll`.
   */
  async webhook(key, { url, types = [], roll = false }) {
    if (!Array.isArray(types) || types.some(x => typeof x !== 'string')) throw fault('invalid_events', 'Event types must be an array of strings', 400);
    return this.store.transact(async tx => {
      const p = await ensure(tx, key);
      const id = `hook:${p.id}`, existing = await tx.get('webhook', id);
      const secret = !existing?.secret || roll ? randomBytes(32).toString('base64url') : null;
      tx.put('webhook', id, { ...existing, id, project: p.id, url, types, enabled: true, secret: secret ? sealText(`webhook:${id}`, secret) : existing.secret });
      tx.emit(p.id, existing ? 'webhook.updated' : 'webhook.created', null, { id });
      return { id, url, types, enabled: true, ...(secret ? { secret } : {}) };
    });
  }
  /** The project's endpoint without its secret, and its latest deliveries; null hook when never set. */
  async webhookConfig(key) {
    const id = `hook:${projectId(key)}`;
    const hook = await this.store.get('webhook', id);
    const deliveries = hook ? (await this.store.list('delivery', { project: projectId(key) })).filter(d => d.hook === id).sort((a, b) => b.at - a.at).slice(0, 20) : [];
    const events = new Map((await this.store.events({ seqs: deliveries.map(d => d.eventSeq) })).map(e => [e.id, e.type]));
    return {
      hook: hook && { id, url: hook.url, types: hook.types, enabled: hook.enabled },
      events: WEBHOOK_EVENTS,
      deliveries: deliveries.map(({ id, state, attempts, at, eventSeq }) => ({ id, state, attempts, at, type: events.get(eventSeq) ?? null })),
    };
  }
  /**
   * The project's Slack sink, as one webhook row with a derived id, so connecting
   * twice replaces the sink instead of stacking duplicates. It carries no secret:
   * the bot token lives in the key's sealed settings and is resolved at send time,
   * which is also what lets one install serve both the OAuth and pasted-token paths.
   */
  async slackSink(key, patch = {}) {
    return this.store.transact(async tx => {
      const p = await ensure(tx, key);
      const id = `slack:${p.id}`;
      const existing = await tx.get('webhook', id);
      // Only what the caller passed changes: the worker disables a dead install
      // without knowing which channel it was pointed at.
      const hook = tx.put('webhook', id, {
        channel: null, types: SLACK_EVENTS, enabled: true, ...existing, ...patch,
        id, project: p.id, kind: 'slack', url: null,
      });
      tx.emit(p.id, existing ? 'webhook.updated' : 'webhook.created', null, { id, kind: 'slack' });
      return { id, channel: hook.channel, types: hook.types, enabled: hook.enabled };
    });
  }
  /**
   * Record an event for a project that already exists. Used by paths outside the
   * control plane's own state machine — SDK runs — so their failures and handover
   * requests reach the same delivery pipeline as session events.
   */
  async emit(key, type, sessionId = null, detail = {}) {
    await this.store.transact(async tx => { tx.emit(projectId(key), type, sessionId, detail); });
  }
}
/** What the Settings endpoint offers to subscribe to; an empty selection means all. */
export const WEBHOOK_EVENTS = [
  'session.ready', 'session.stopped', 'session.failed', 'session.disconnected',
  'run.needs_attention', 'run.failed', 'budget.threshold',
  'persona.created', 'persona.updated', 'persona.deleted',
  'recording.ready', 'login.completed', 'login.failed', 'mfa.completed',
  'credential.created', 'credential.revoked',
];
/** What a Slack sink subscribes to when it is created. */
export const SLACK_EVENTS = ['run.needs_attention', 'run.failed', 'session.failed'];
let singleton;
export const control = () => singleton ||= new ControlService();
