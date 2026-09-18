import { control, projectId, instanceId } from './control/service.js';
import { openRelay } from './cdp-relay.js';
import { forwardGateway } from './control/cluster.js';
/**
 * CDP gateway.
 *
 * Serves Chrome's discovery endpoint and a WebSocket that speaks raw CDP, so
 * Playwright, Puppeteer, Stagehand, browser-use and any other CDP client
 * connect to this control plane as if it were a browser — no client changes,
 * no vendor SDK. The gateway picks a provider by the configured routing
 * strategy, fails over if one is down, and queues when everything is busy.
 *
 * The wire is forwarded verbatim. Features that need to observe it (recording,
 * profile capture) use a second, independent CDP connection to the same
 * browser rather than injecting frames into the client's session, so a
 * client's own use of Page.screencast or Network is never disturbed.
 *
 * ponytail: every message is forwarded through JS rather than piped at the
 * socket level. Measured cost is a JSON-free buffer copy per frame; if a
 * profile of a saturated gateway ever shows this dominating, add a raw
 * passthrough for sessions with no features enabled.
 */

import { WebSocketServer, WebSocket } from 'ws';
import { randomUUID } from 'crypto';
import { validateApiKey, authenticateToken } from './auth.js';
import { pool } from './routing.js';
import * as keyConfig from './key-config.js';
import { acquire as acquireProvider } from './providers.js';
import { metrics } from './metrics.js';
import { audit } from './audit.js';
import * as usage from './usage.js';
import { consume, checkQuota, QUOTAS } from './limits.js';
import { fingerprint } from './audit.js';
import * as profiles from './profiles.js';
import * as recorder from './recorder.js';
import { registry } from './connection-registry.js';

/** Live gateway sessions, keyed by session id. */
export const sessions = new Map();

const GRACE_MS = Number(process.env.OYA_SESSION_GRACE_MS) || 60_000;

function chromeVersion() {
  return {
    Browser: 'Chrome/126.0.0.0',
    'Protocol-Version': '1.3',
    'User-Agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 '
      + '(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36',
    'V8-Version': '12.6.228.9',
    'WebKit-Version': '537.36',
  };
}

/**
 * CDP discovery. Playwright and Puppeteer fetch this first and then dial
 * webSocketDebuggerUrl, which is why pointing them at the gateway just works.
 */
export function handleJsonVersion(req, res) {
  const host = req.headers.host || 'localhost';
  const scheme = (req.headers['x-forwarded-proto'] || '').includes('https') ? 'wss' : 'ws';
  const token = new URL(req.url, `http://${host}`).searchParams.get('token');
  res.json({
    ...chromeVersion(),
    webSocketDebuggerUrl: `${scheme}://${host}/connect${token ? `?token=${encodeURIComponent(token)}` : ''}`,
  });
}

/** Some clients probe /json/list before connecting. */
export async function handleJsonList(req, res) {
  let principal;
  try { principal = await authenticateToken(req.headers.authorization?.slice(7)); }
  catch (e) { return res.status(e.status || 503).json({ error: e.message }); }
  const host = req.headers.host || 'localhost';
  const scheme = (req.headers['x-forwarded-proto'] || '').includes('https') ? 'wss' : 'ws';
  res.json([...sessions.values()].filter(s => s.apiKey === principal.key).map((s) => ({
    id: s.id,
    type: 'page',
    title: s.profile ? `Gateway session (${s.profile})` : 'Gateway session',
    url: 'about:blank',
    webSocketDebuggerUrl: `${scheme}://${host}/connect?session=${s.id}`,
  })));
}

export const wss = new WebSocketServer({ noServer: true, perMessageDeflate: false, maxPayload: 256 * 1024 * 1024 });

/**
 * A gateway session. Owns the upstream CDP connection and survives a client
 * disconnect for GRACE_MS so a dropped client can resume against the same
 * provider with its page state intact.
 */
// Calibration: long enough for ordinary waits (Playwright defaults to 30s), short enough that takeover is never stuck.
const STUCK_COMMAND_MS = Number(process.env.OYA_STUCK_COMMAND_MS) || 60000;

class Session {
  constructor({ id, apiKey, provider, release, upstream, profile }) {
    Object.assign(this, { id, apiKey, provider, release, upstream, profile });
    // Profiles and recordings are namespaced by this, never by the raw key.
    this.owner = fingerprint(apiKey);
    this.client = null;
    this.startedAt = Date.now();
    this.bytesUp = 0;
    this.bytesDown = 0;
    this.graceTimer = null;
    this.closed = false;
    this.pendingToClient = [];
    this.commandTail = Promise.resolve();
    this.commandReleases = new Map();
  }

  attach(client) {
    clearTimeout(this.graceTimer);
    this.graceTimer = null;
    this.client = client;

    // Anything the browser said while nobody was listening is delivered on
    // resume rather than lost.
    for (const buf of this.pendingToClient.splice(0)) {
      try { client.send(buf); } catch {}
    }

    client.on('message', (data, isBinary) => {
      this.commandTail = this.commandTail.then(async () => {
        if (this.closed || this.client !== client) return;
        // Revocation is enforced by the attachment validator every two seconds, not per CDP message.
        const command = JSON.parse(data.toString());
        if (command.id === undefined) throw new Error('CDP command ID is required');
        const commandKey = `${command.sessionId || ''}:${command.id}`;
        if (this.commandReleases.has(commandKey)) throw new Error('Duplicate CDP command ID');
        const settle = await control().beginCommand(this.attachedTo || this.id);
        // A command the browser never answers must not block human takeover forever.
        const timer = setTimeout(() => this.settle(commandKey), STUCK_COMMAND_MS);
        const finish = () => { clearTimeout(timer); return settle(); };
        this.commandReleases.set(commandKey, finish);
        this.bytesUp += data.length;
        if (this.upstream.readyState !== WebSocket.OPEN) {
          this.commandReleases.delete(commandKey); await finish(); throw new Error('Browser disconnected');
        }
        this.upstream.send(data, { binary: isBinary });
      }).catch(() => client.close(1008, 'Session access paused, revoked, or command invalid'));
    });

    client.on('close', () => {
      if (this.client !== client) return;
      this.client = null;
      if (this.closed) return;
      // Hold the browser briefly so a reconnect resumes the same session.
      this.graceTimer = setTimeout(() => this.destroy('grace expired'), GRACE_MS);
      metrics.gatewaySessions.set({}, sessions.size);
    });

    client.on('error', () => {});
  }

  /** Release a command's in-flight slot: on its reply, or once it has run for STUCK_COMMAND_MS. */
  settle(key) {
    const finish = this.commandReleases.get(key);
    if (finish) { this.commandReleases.delete(key); void finish().catch(() => {}); }
  }

  bindUpstream() {
    this.upstream.on('message', (data, isBinary) => {
      // Only replies release command slots; skip parsing event traffic when nothing is outstanding.
      if (this.commandReleases.size) try {
        const reply = JSON.parse(data.toString());
        this.settle(`${reply.sessionId || ''}:${reply.id}`);
      } catch {}
      this.bytesDown += data.length;
      if (this.client?.readyState === WebSocket.OPEN) {
        this.client.send(data, { binary: isBinary });
      } else if (this.pendingToClient.length < 1000) {
        this.pendingToClient.push(data);
      }
    });
    this.upstream.on('close', () => this.destroy('provider closed'));
    this.upstream.on('error', () => this.destroy('provider error'));
  }

  async destroy(reason) {
    if (this.closed) return;
    this.closed = true;
    await Promise.allSettled([...this.commandReleases.values()].map(finish => finish()));
    this.commandReleases.clear();
    clearTimeout(this.graceTimer);
    sessions.delete(this.id);
    if (this.attachedTo) await control().store.transact(async tx => { if (await tx.get('attachment', this.id)) await tx.delete('attachment', this.id); }).catch(() => {});

    if (await recorder.stop(this.id).catch(() => false)) void control().emit(this.apiKey, 'recording.ready', this.id, {}).catch(() => {});
    if (this.profile) {
      await profiles.capture(this.owner, this.profile, this)
        .catch((e) => console.error(`[gateway] profile capture failed for ${this.profile}:`, e.message));
    }
    // Held open since restore so its on-new-document hook stays registered.
    try { this.profileConn?.close(); } catch {}

    try { this.client?.close(1001, reason); } catch {}
    try { this.upstream?.close(); } catch {}
    if (!this.attachedTo) await control().update(this.apiKey, this.id, { state: 'cleanup_pending' }).catch(() => {});
    try {
      await this.release?.();
      if (!this.attachedTo) await control().update(this.apiKey, this.id, { state: 'stopped' });
    } catch (err) { console.error('[gateway] cleanup pending:', err.message); }

    const seconds = Math.round((Date.now() - this.startedAt) / 1000);
    if (!this.attachedTo) usage.record(this.apiKey, 'browser_seconds', seconds);
    usage.record(this.apiKey, 'bytes_out', this.bytesDown);
    metrics.gatewaySessions.set({}, sessions.size);
    metrics.gatewaySessionDuration.observe({ provider: this.provider }, seconds * 1000);
    audit({
      action: 'gateway.session.end', actorKey: this.apiKey, targetType: 'session', targetId: this.id,
      meta: { provider: this.provider, seconds, reason, profile: this.profile || null },
    });
  }

  toJSON() {
    return {
      id: this.id, provider: this.provider, profile: this.profile || null, attachedTo: this.attachedTo || null,
      connected: !!this.client, startedAt: new Date(this.startedAt).toISOString(),
      seconds: Math.round((Date.now() - this.startedAt) / 1000),
      bytesUp: this.bytesUp, bytesDown: this.bytesDown,
      recording: recorder.isRecording(this.id),
    };
  }
}

/** Route an HTTP upgrade on /connect into a gateway session. */
export async function handleUpgrade(req, socket, head) {
  const url = new URL(req.url, `http://${req.headers.host}`);
  let token = url.searchParams.get('token')
    || (req.headers.authorization?.startsWith('Bearer ') ? req.headers.authorization.slice(7) : '');

  const deny = (code, message) => {
    socket.write(`HTTP/1.1 ${code} ${message}\r\nConnection: close\r\n\r\n`);
    socket.destroy();
  };

  let principal;
  const ticket = url.searchParams.get('ticket');
  let authToken = token;
  try {
    if (ticket) {
      token = await control().redeem(ticket, url.searchParams.get('browser') || url.searchParams.get('session'));
      principal = await authenticateToken(token);
      authToken = token;
      token = principal.key;
    } else {
      if (url.searchParams.has('token') && process.env.OYA_ALLOW_LEGACY_QUERY_KEYS === 'false') return deny(401, 'Use a connection ticket');
      principal = await authenticateToken(token);
      token = principal.key;
    }
    if (principal.role === 'viewer') return deny(403, 'Operator permission required');
  } catch (e) { return deny(e.status === 503 ? 503 : 401, 'Unauthorized'); }

  const targetId = url.searchParams.get('browser') || url.searchParams.get('session');
  if (targetId && await forwardGateway(req, socket, head, authToken || token, token, targetId)) return;

  if (!consume('connect', token).allowed) {
    metrics.gatewayConnects.inc({ outcome: 'rate_limited' });
    return deny(429, 'Too Many Requests');
  }

  // ── Resume an existing session ──
  const resumeId = url.searchParams.get('session');
  if (resumeId) {
    const existing = sessions.get(resumeId);
    if (!existing) { metrics.gatewayConnects.inc({ outcome: 'unknown_session' }); return deny(404, 'Not Found'); }
    if (existing.apiKey !== token) {
      metrics.gatewayConnects.inc({ outcome: 'forbidden' });
      return deny(403, 'Forbidden');
    }
    if (existing.client) { metrics.gatewayConnects.inc({ outcome: 'session_busy' }); return deny(409, 'Session In Use'); }
    return wss.handleUpgrade(req, socket, head, (client) => {
      existing.authToken = authToken || token;
      existing.attach(client);
      metrics.gatewayConnects.inc({ outcome: 'resumed' });
      audit({ action: 'gateway.session.resume', actorKey: token, targetType: 'session', targetId: existing.id, req });
    });
  }

  // ── Attach to a browser already in the fleet ──
  //
  // "Connect Playwright to *this* browser" from the console. The registry
  // browser stays where it is; this is a second CDP client on the same
  // upstream, which Chrome allows. An Oya client has no endpoint to dial, so its
  // CDP is relayed over the socket it dialled us on (cdp-relay.js) — if it
  // enrolled with its front door on.
  const attachId = url.searchParams.get('browser');
  if (attachId) {
    const target = registry.get(attachId);
    if (!target || target.apiKey !== token) {
      metrics.gatewayConnects.inc({ outcome: 'unknown_browser' });
      return deny(404, 'Not Found');
    }
    if (!target.driver?.wsUrl && !target.cdp) {
      metrics.gatewayConnects.inc({ outcome: 'not_attachable' });
      return deny(409, 'Not Attachable');
    }
    let upstream;
    try {
      if (!target.driver?.wsUrl) upstream = await openRelay(target, attachId);
      else {
        upstream = new WebSocket(target.driver.wsUrl, { maxPayload: 256 * 1024 * 1024, handshakeTimeout: 20_000 });
        await new Promise((resolve, reject) => {
          const t = setTimeout(() => reject(new Error('upstream connect timed out')), 20_000);
          upstream.once('open', () => { clearTimeout(t); resolve(); });
          upstream.once('error', (e) => { clearTimeout(t); reject(e); });
        });
      }
    } catch (err) {
      metrics.gatewayConnects.inc({ outcome: 'attach_failed' });
      audit({ action: 'gateway.connect', actorKey: token, outcome: 'error', targetType: 'browser', targetId: attachId, meta: { error: err.message }, req });
      return deny(502, 'Bad Gateway');
    }
    const id = randomUUID();
    const session = new Session({
      id, apiKey: token, provider: target.provider || 'cdp', profile: null, upstream,
      // Nothing to release: the browser belongs to the registry, and stays.
      release: () => {},
    });
    session.upstreamUrl = target.driver?.wsUrl || `relay:${attachId}`;
    session.attachedTo = attachId;
    await control().store.transact(async tx => { tx.put('attachment', session.id, { id: session.id, project: projectId(token), instance: instanceId, leaseUntil: Date.now() + 120000, browserId: attachId }); });
    session.authToken = authToken || token;
    session.bindUpstream();
    sessions.set(id, session);
    return wss.handleUpgrade(req, socket, head, (client) => {
      session.attach(client);
      metrics.gatewayConnects.inc({ outcome: 'attached' });
      metrics.gatewaySessions.set({}, sessions.size);
      audit({ action: 'gateway.session.attach', actorKey: token, targetType: 'browser', targetId: attachId,
        meta: { session: id, provider: target.provider }, req });
    });
  }

  if (registry.draining || (await control().store.get('meta', 'draining'))?.value) return deny(503, 'Server draining');

  // ── New session ──
  const profileName = url.searchParams.get('profile');
  const owner = fingerprint(token);
  if (profileName) {
    const lock = profiles.tryLock(owner, profileName);
    if (!lock.ok) {
      // Two browsers sharing one jar corrupts it, so the second is refused
      // rather than silently racing.
      metrics.gatewayConnects.inc({ outcome: 'profile_busy' });
      audit({ action: 'gateway.connect', actorKey: token, outcome: 'denied', targetType: 'profile',
        targetId: profileName, meta: { reason: 'profile in use' }, req });
      return deny(409, 'Profile In Use');
    }
  }

  const mine = [...sessions.values()].filter((s) => s.apiKey === token).length;
  const quota = checkQuota('browsers', token, mine);
  if (!quota.allowed) {
    if (profileName) profiles.unlock(owner, profileName);
    metrics.gatewayConnects.inc({ outcome: 'quota' });
    return deny(429, `Browser quota reached (${quota.quota})`);
  }

  let acquired, reservation;
  try {
    reservation = await control().reserve(token, { provider: 'gateway', maxConcurrent: QUOTAS.browsers, request: { profile: profileName }, persona: profileName ? `profile:${profileName}` : null, personaLimit: 1 });
    acquired = await pool.acquire({
      // This key's own providers plus whatever the host shares.
      owner,
      strategy: url.searchParams.get('strategy') || undefined,
      connect: async (provider) => {
        const target = provider.type === 'cdp' && provider.wsUrl
          ? { wsUrl: provider.wsUrl, provider: provider.name, sessionId: null, release: async () => {} }
          : await acquireProvider({ provider: provider.type, env: provider.owner === null ? process.env : keyConfig.envFor(token), onCreated: cleanup => control().update(token, reservation.id, { cleanup }) });
        let upstream;
        try {
          upstream = new WebSocket(target.wsUrl, { maxPayload: 256 * 1024 * 1024, handshakeTimeout: 20_000 });
          await new Promise((resolve, reject) => {
            upstream.once('open', resolve);
            upstream.once('error', reject);
          });
          return { upstream, target };
        } catch {
          upstream?.terminate();
          await target.release().catch((err) => console.error('[gateway] cleanup:', err.message));
          throw new Error('Provider browser connection failed');
        }
      },
    });
  } catch (err) {
    if (profileName) profiles.unlock(owner, profileName);
    // Admission refusals (quota, drain, policy) are the caller's answer, not a provider failure.
    if (!reservation) {
      metrics.gatewayConnects.inc({ outcome: 'quota' });
      return deny(err.status || 503, err.status && err.status < 500 ? err.message : 'Service Unavailable');
    }
    await control().complete(token, reservation.id, err.status || 502, { error: 'Provider acquisition failed' }).catch(() => {});
    metrics.gatewayConnects.inc({ outcome: 'no_provider' });
    audit({ action: 'gateway.connect', actorKey: token, outcome: 'error', meta: { error: err.message }, req });
    return deny(err.status === 503 ? 503 : 502, err.status === 503 ? 'Service Unavailable' : 'Bad Gateway');
  }

  const { provider, session: { upstream, target }, release, holdId } = acquired;
  const id = reservation.id;
  try {
    await control().store.transact(async tx => { const hold = await tx.get('hold', holdId); if (hold) hold.sessionId = id; });
    // provider stays 'gateway' so lifecycle rules still know it is attach-only; vendor records where it came from.
    await control().update(token, id, { state: 'ready', provisioningActive: false, vendor: provider.name, cleanup: target.cleanup || null });
  } catch (err) {
    // Stopped while acquiring, or storage lost: hand the browser back rather than leak it.
    upstream.terminate();
    await Promise.allSettled([target.release?.(), release()]);
    if (profileName) profiles.unlock(owner, profileName);
    return deny(err.status === 409 ? 409 : 503, err.status === 409 ? 'Conflict' : 'Service Unavailable');
  }
  const session = new Session({
    id, apiKey: token, provider: provider.name, profile: profileName || null,
    upstream,
    release: async () => {
      await target.release?.();
      await release();
      if (profileName) profiles.unlock(owner, profileName);
    },
  });
  session.authToken = authToken || token;
  session.upstreamUrl = target.wsUrl;
  session.bindUpstream();
  sessions.set(id, session);

  // Restore before the client can navigate, so the first page load already has
  // the profile's cookies.
  if (profileName) await profiles.restore(owner, profileName, session).catch((e) => console.error('[gateway] profile restore:', e.message));
  if (url.searchParams.get('record') === '1') {
    try { await recorder.start(session); }
    catch (e) { await session.destroy('Recording unavailable'); return deny(e.status || 503, 'Recording unavailable'); }
  }

  wss.handleUpgrade(req, socket, head, (client) => {
    session.attach(client);
    usage.record(token, 'browsers_started');
    metrics.gatewayConnects.inc({ outcome: 'ok' });
    metrics.gatewaySessions.set({}, sessions.size);
    audit({ action: 'gateway.session.start', actorKey: token, targetType: 'session', targetId: id,
      meta: { provider: provider.name, profile: profileName || null, recording: url.searchParams.get('record') === '1' }, req });
  });
}

export function listSessions(apiKey, { all = false } = {}) {
  return [...sessions.values()].filter((s) => all || s.apiKey === apiKey).map((s) => s.toJSON());
}

export async function killSession(id, reason = 'closed by operator') {
  const s = sessions.get(id);
  if (!s) return false;
  await s.destroy(reason);
  return true;
}
