import https from 'node:https';
import { desktopState } from './desktop.js';
import { isIP } from 'node:net';
import { createHmac, randomUUID } from 'node:crypto';
import { heartbeatInstance, clusterOrigin } from './cluster.js';
import { control, instanceId, terminal, attachOnly, holdsSlot, live, keyOfProject } from './service.js';
import { openText } from '../secrets.js';
import { registry } from '../connection-registry.js';
import { sessions as gateways } from '../gateway.js';
import { releasePersisted } from '../providers.js';
import { removeManaged } from './managed.js';
import { removeSandbox } from '../sandbox.js';
import { assertSafeTarget } from '../net-guard.js';
import { maintain as maintainRecordings } from '../recorder.js';
import { metrics } from '../metrics.js';
import { authenticateToken } from '../auth.js';
import * as slack from '../slack.js';
import * as keyConfig from '../key-config.js';

const DAY = 86400000, METER_MS = 30000;
let timer, accessTimer, leaseTimer, heartbeating = false, running = false, validating = false, provisioning = null, maintenance = null, lastMaintenance = 0;
export const workerHealth = { lastSuccess: null, lastError: null, pendingCleanup: 0, pendingWebhooks: 0 };
export async function tick(service = control()) {
  if (running) return;
  running = true;
  try {
    const now = Date.now();
    // Lease renewal and human-control expiry for local sessions belong to renewLeases, which a slow cleanup cannot delay.
    const { jobs, sessions } = await service.store.transact(async tx => {
      // Renew well before expiry so an idle tick commits nothing; expired rows are pruned by maintenance.
      for (const a of await tx.list('attachment', { states: [instanceId] })) if (gateways.has(a.id) && a.leaseUntil < now + 90000) a.leaseUntil = now + 120000;
      for (const h of await tx.list('hold')) if (gateways.has(h.sessionId) && h.expiresAt < now + 120000) h.expiresAt = now + 180000;
      const sessions = await tx.list('session', { states: live }), jobs = [];
      for (const x of sessions) {
        const prior = x.state, attached = registry.get(x.id) || gateways.get(x.id);
        // A lost attachment cannot return under its ID; a dial-in browser can.
        if (x.state === 'ready' && !attached && x.leaseUntil < now) x.state = x.cleanup ? 'cleanup_pending' : ['cdp', 'gateway'].includes(x.provider) ? 'stopped' : 'disconnected';
        if (['provisioning', 'unknown_outcome'].includes(x.state) && x.leaseUntil < now) x.state = x.cleanup ? 'cleanup_pending' : attachOnly.has(x.provider) ? 'failed' : 'unknown_outcome';
        if (x.state !== prior) tx.emit(x.project, `session.${x.state}`, x.id);
        if (jobs.length >= 8 || x.state !== 'cleanup_pending' || x.nextCleanupAt > now || x.cleanupLease > now || (x.provisioningActive && x.leaseUntil > now)) continue;
        // The live owner closes its own connection; another replica takes over once that lease lapses.
        if (x.instance !== instanceId && x.leaseUntil > now) continue;
        x.cleanupLease = now + 120000;
        x.fence += 1;
        jobs.push({ ...x });
      }
      return { jobs, sessions: sessions.map(({ id, project, state, updatedAt, rateUsdHour }) => ({ id, project, state, updatedAt, rateUsdHour })) };
    });
    const keys = new Map((await service.store.load([...new Set(jobs.map(x => x.project))].map(id => ({ kind: 'project', id })))).flat().map(r => [r.id, r.body.key]));
    for (const x of jobs) {
      let key;
      // One project sealed under another secret must not stall every other project's cleanup; its lease lapses and it retries.
      try { key = openText(`control:${x.project}`, keys.get(x.project)); } catch { workerHealth.lastError = `Project ${x.project} key could not be decrypted`; continue; }
      try {
        if (x.cleanup?.kind === 'docker') await removeManaged(x.cleanup.container, key, x.id, x.cleanup.daemonId, x.cleanup.runtime, x.cleanup.namespace);
        else if (x.cleanup?.kind === 'sandbox') await removeSandbox(x.cleanup.browserId, key);
        else if (x.cleanup?.kind === 'vendor') await releasePersisted(x.cleanup);
        else if (!attachOnly.has(x.provider) && !registry.get(x.id) && !gateways.get(x.id)) throw new Error('Resource outcome is unknown; operator reconciliation required');
        if (gateways.has(x.id)) await gateways.get(x.id).destroy('Stopped by control plane');
        if (registry.get(x.id)) { try { registry.get(x.id).ws?.close(4008, 'Stopped by control plane'); } catch {} registry.remove(x.id); }
        await service.update(key, x.id, { state: 'stopped', cleanupLease: null }, { fence: x.fence });
      } catch (err) {
        await service.update(key, x.id, { cleanupLease: null, cleanupAttempts: (x.cleanupAttempts || 0) + 1, nextCleanupAt: now + Math.min(300000, 1000 * 2 ** Math.min(8, x.cleanupAttempts || 0)), cleanupError: 'Provider cleanup failed; retry pending' }, { fence: x.fence }).catch(() => {});
      }
    }
    // Only projects with rated sessions accrue cost; budgets require a rate card, so this also covers every budget.
    for (const project of new Set(sessions.filter(x => x.rateUsdHour != null && holdsSlot(x)).map(x => x.project))) await meter(service, project);
    if (!provisioning) { provisioning = provisionQueued(service).catch(err => { workerHealth.lastError = err.message; }).finally(() => { provisioning = null; }); }
    workerHealth.pendingWebhooks = await deliver(service);
    const cleanup = sessions.filter(x => x.state === 'cleanup_pending');
    workerHealth.pendingCleanup = cleanup.length;
    metrics.controlCleanup.set({}, cleanup.length);
    metrics.controlWebhooks.set({}, workerHealth.pendingWebhooks);
    metrics.controlQueue.set({}, sessions.filter(x => x.state === 'queued').length);
    metrics.controlCleanupAge.set({}, Math.max(0, ...cleanup.map(x => (Date.now() - x.updatedAt) / 1000)));
    if (!maintenance && Date.now() - lastMaintenance > 60000) { lastMaintenance = Date.now(); maintenance = maintainControl(service).catch(e => console.error('[control] maintenance:', e.message)).finally(() => { maintenance = null; }); }
    workerHealth.lastSuccess = Date.now(); workerHealth.lastError = null;
  } catch (err) { workerHealth.lastError = err.message; }
  finally { running = false; }
}
/** Accrue estimated cost into the session and its project, then enforce session and project budgets. */
async function meter(service, id) {
  await service.store.transact(async tx => {
    const now = Date.now(), p = await tx.get('project', id);
    if (!p) return;
    const sessions = await tx.list('session', { project: id, states: live }), stop = new Set();
    for (const x of sessions) {
      const since = x.meteredAt || x.createdAt;
      if (!holdsSlot(x) || x.rateUsdHour == null || now - since < METER_MS) continue;
      const cost = (now - since) / 3600000 * x.rateUsdHour;
      x.costUsd += cost; p.costUsd = (p.costUsd || 0) + cost; x.meteredAt = now;
    }
    for (const x of sessions) if (x.budgetUsd != null && holdsSlot(x) && x.costUsd + (x.reservedCostUsd || 0) >= x.budgetUsd) stop.add(x);
    const budget = p.settings.budgetUsd, spent = p.costUsd || 0;
    if (budget != null) {
      for (const threshold of [0.8, 1]) {
        const alert = `${budget}:${threshold}`;
        if (spent >= budget * threshold && !p.alerts?.[alert]) { (p.alerts ||= {})[alert] = true; tx.emit(id, 'budget.threshold', null, { threshold, estimatedUsd: spent }); }
      }
      // Queued work has no resource yet; it waits for budget or its deadline instead of entering cleanup.
      if (spent + sessions.reduce((n, x) => n + (x.reservedCostUsd || 0), 0) >= budget) for (const x of sessions) if (x.managed && holdsSlot(x)) stop.add(x);
    }
    for (const x of stop) if (x.state !== 'cleanup_pending') { x.state = 'cleanup_pending'; tx.emit(id, 'session.cleanup_pending', x.id, { reason: 'budget' }); }
  });
}
/** Retention: expired rows, events past each project's audit window, credentials of ended managed browsers, recordings. */
export async function maintainControl(service = control()) {
  const now = Date.now();
  const projects = await service.store.list('project');
  await service.store.prune(now, Object.fromEntries(projects.map(p => [p.id, now - (p.settings.auditDays || 90) * DAY])));
  const enrolled = await service.store.list('credential', { states: ['browser'] });
  if (enrolled.length) {
    const running = new Set((await service.store.load(enrolled.map(c => ({ kind: 'session', id: String(c.sessionId) })))).flat().filter(r => !terminal.has(r.body.state)).map(r => r.id));
    const ended = enrolled.filter(c => !running.has(c.sessionId)).map(c => c.digest);
    if (ended.length) await service.store.transact(async tx => { await tx.getMany('credential', ended); for (const digest of ended) await tx.delete('credential', digest); });
  }
  await maintainRecordings();
}
async function provisionQueued(service) {
  const host = new URL(process.env.OYA_PUBLIC_WS_URL || 'ws://localhost:3100').host;
  // ponytail: sequential, up to eight per tick; run claims in parallel if queue drain rate matters.
  for (let i = 0; i < 8; i++) {
    const job = await service.claimQueued();
    if (!job) return;
    const { startBrowser } = await import('../api.js');
    const req = { headers: { authorization: `Bearer ${job.key}`, host }, body: job.request, controlSession: job.session, socket: {}, secure: process.env.OYA_PUBLIC_WS_URL?.startsWith('wss:') };
    let status = 200, response;
    const res = { status(code) { status = code; return this; }, json(body) { response = body; return this; } };
    try {
      await startBrowser(req, res);
      await service.complete(job.key, job.session.id, status, response);
    } catch { await service.update(job.key, job.session.id, { state: 'unknown_outcome' }, { fence: job.session.fence }).catch(() => {}); }
  }
}
/** Claims up to eight due deliveries and sends them; returns how many deliveries were pending. */
export async function deliver(service, sender = sendWebhook) {
  const claim = randomUUID(), now = Date.now();
  const { batch, pending } = await service.store.transact(async tx => {
    const pending = await tx.list('delivery', { states: ['pending'] });
    const due = pending.filter(d => d.nextAt <= now && !(d.leaseUntil > now)).slice(0, 8);
    const hooks = new Map((await tx.getMany('webhook', due.map(d => d.hook))).map(h => [h.id, h]));
    const batch = [];
    for (const d of due) {
      const hook = hooks.get(d.hook);
      if (!hook?.enabled) { d.state = 'cancelled'; continue; }
      if (now - (d.replayAt || d.at) > DAY) { d.state = 'failed'; continue; }
      Object.assign(d, { claim, leaseUntil: now + 60000, attempts: d.attempts + 1 });
      batch.push({ ...d, hook });
    }
    return { batch, pending: pending.length };
  });
  const events = new Map((await service.store.events({ seqs: batch.map(d => d.eventSeq) })).map(e => [e.id, e]));
  await Promise.all(batch.map(async d => {
    let ok = false, dead = false;
    try {
      const event = events.get(d.eventSeq);
      if (!event) throw new Error('Event is past retention');
      if (d.hook.kind === 'slack') ({ ok, dead } = await postSlack(service, d.hook, event));
      else {
        const body = JSON.stringify(event), timestamp = String(Math.floor(Date.now() / 1000));
        const signature = createHmac('sha256', openText(`webhook:${d.hook.id}`, d.hook.secret)).update(`${timestamp}.${body}`).digest('hex');
        ok = await sender(d.hook.url, body, { 'Content-Type': 'application/json', 'Oya-Event-Id': String(event.id), 'Oya-Signature': `t=${timestamp},v1=${signature}` });
      }
    } catch { /* delivery state retains the retry obligation */ }
    await service.store.transact(async tx => {
      const current = await tx.get('delivery', d.id);
      if (current?.claim !== claim) return;
      // A revoked token or a deleted channel is not a transient failure: retrying it
      // every backoff step for a day would bury the queue behind an install that is gone.
      if (dead) {
        Object.assign(current, { state: 'cancelled', leaseUntil: null });
        const hook = await tx.get('webhook', d.hook.id);
        if (hook) hook.enabled = false;
        return;
      }
      Object.assign(current, { state: ok ? 'delivered' : 'pending', leaseUntil: null, nextAt: Date.now() + Math.min(3600000, 1000 * 2 ** Math.min(d.attempts, 12)) });
    });
  }));
  return pending;
}
/**
 * One event as a Slack message. The bot token lives in the project key's sealed
 * settings rather than on the hook row, so an OAuth install and a pasted token
 * arrive here identically. The live link is minted per message and expires in an
 * hour: a share credential scoped to that one browser, which is what makes the
 * alert actionable for someone with no Oya account.
 */
async function postSlack(service, hook, event) {
  const key = await keyOfProject(hook.project, service);
  if (!key) return { ok: false, dead: false };
  const install = keyConfig.getSlack(key);
  const channel = hook.channel || install?.channelId;
  // Settings sealed under a rotated secret read as absent, which is recoverable —
  // retry rather than disabling a sink the customer never touched. Disconnecting
  // disables the hook itself, and those deliveries are cancelled before they reach here.
  if (!install?.botToken || !channel) return { ok: false, dead: false };
  let liveUrl = null;
  if (event.sessionId) {
    // Fails for a browser that has already ended — then the message goes out without the button.
    const share = await service.share(key, { id: event.sessionId, control: true, expiresIn: 3600 }).catch(() => null);
    if (share) liveUrl = `${slack.consoleUrl()}/live/${encodeURIComponent(event.sessionId)}#t=${encodeURIComponent(share.token)}`;
  }
  const result = await slack.call(install.botToken, 'chat.postMessage', { channel, unfurl_links: false, ...slack.blocksFor(event, liveUrl) });
  return { ok: !!result?.ok, dead: !result?.ok && slack.isDeadInstall(result?.error) };
}
async function sendWebhook(url, body, headers) {
  const target = await assertSafeTarget(url, { protocols: ['https:'], label: 'webhook URL' });
  const address = target.addresses[0];
  return new Promise((resolve, reject) => {
    const request = https.request(url, {
      method: 'POST', headers, signal: AbortSignal.timeout(10000),
      // Pin the address checked above while retaining the original TLS hostname.
      lookup: (hostname, options, callback) => options.all
        ? callback(null, [{ address, family: isIP(address) }])
        : callback(null, address, isIP(address)),
    }, response => { response.resume(); resolve(response.statusCode >= 200 && response.statusCode < 300); });
    request.once('error', reject); request.end(body);
  });
}
async function validateAttachments() {
  if (validating) return;
  validating = true;
  // One check per credential per round. A revoked credential closes with 4003 (clients stop); an outage uses 1013 so desktop browsers reconnect.
  const verdicts = new Map();
  const closeCode = (token, allowBrowser = false) => {
    if (!verdicts.has(token)) verdicts.set(token, authenticateToken(token, { allowBrowser }).then(() => null, e => e.status === 503 ? 1013 : 4003));
    return verdicts.get(token);
  };
  try {
    await Promise.all([...gateways.values()].map(async session => {
      if (await closeCode(session.authToken || session.apiKey)) session.client?.close(1008, 'Credential revoked or validation unavailable');
    }));
    await Promise.all([...registry.browsers.entries()].map(async ([browserId, browser]) => {
      const code = await closeCode(browser.authToken || browser.apiKey, true);
      if (code) { browser.ws?.close(code, 'Credential revoked or validation unavailable'); for (const viewer of browser.streamViewers) viewer.end(); }
      await Promise.all([...browser.streamViewers].map(async viewer => {
        if (await closeCode(viewer.authToken || browser.apiKey)) { viewer.end(); registry.removeViewer(browserId, viewer); }
      }));
    }));
  } finally { validating = false; }
}
async function renewLeases() {
  if (heartbeating) return;
  heartbeating = true;
  try {
    const modes = await control().store.transact(async tx => {
      await heartbeatInstance(tx);
      const now = Date.now(), modes = [];
      // Only this replica's own attachments are read; the fleet is never scanned for renewal.
      for (const x of await tx.getMany('session', [...registry.browsers.keys(), ...gateways.keys()])) {
        if (x.instance !== instanceId || terminal.has(x.state)) continue;
        if (x.leaseUntil < now + 20000) x.leaseUntil = now + 30000;
        if (x.control.mode === 'human' && x.control.expiresAt <= now) { x.control = { mode: 'paused', revision: Math.max(now, (x.control.revision || 0) + 1) }; tx.emit(x.project, 'control.paused', x.id); }
        modes.push([x.id, desktopState(x.id, x.control)]);
      }
      return modes;
    });
    for (const [id, state] of modes) {
      const ws = registry.get(id)?.ws;
      if (ws?.readyState === 1) ws.send(JSON.stringify({ type: 'control_mode', mode: state.mode, state }));
    }
  } finally { heartbeating = false; }
}
export function startWorkers() {
  clusterOrigin(); // fail fast on an unroutable cluster configuration
  timer ||= setInterval(() => void tick(), 2000); timer.unref();
  accessTimer ||= setInterval(() => void validateAttachments().catch(() => {}), 2000); accessTimer.unref();
  leaseTimer ||= setInterval(() => void renewLeases().catch(e => { workerHealth.lastError = e.message; }), 2000); leaseTimer.unref();
  void tick();
  // Resolves once this replica is advertised and routable.
  return renewLeases().catch(e => { workerHealth.lastError = e.message; });
}
export async function stopWorkers() {
  clearInterval(timer); clearInterval(accessTimer); clearInterval(leaseTimer); timer = null; accessTimer = null; leaseTimer = null;
  while (running || validating || heartbeating) await new Promise(r => setTimeout(r, 50));
  await provisioning; await maintenance;
}
