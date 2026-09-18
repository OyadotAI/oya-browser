import { control, hash } from './control/service.js';
import { desktopControl } from './control/desktop.js';
/**
 * WebSocket handler — manages browser connections, auth, ping/pong, and command dispatch.
 */

import { v4 as uuidv4 } from 'uuid';
import { validateApiKey, authenticateToken } from './auth.js';
import { registry, summarise } from './connection-registry.js';
import { destroyMcpServer } from './mcp-server.js';
import { mergeDump, applyChange, getAll as getAllCookies, getForDomains, getStorage, mergeStorage, drain as drainLogins, summary as loginSummary } from './cookie-store.js';
import { metrics } from './metrics.js';
import * as usage from './usage.js';
import * as personas from './personas.js';
import * as proxies from './proxies.js';
import * as keyConfig from './key-config.js';
import { isProvisioned } from './sandbox.js';
import { onBrowserMessage as onRelayMessage, closeRelays } from './cdp-relay.js';
import { fingerprint as personaOwner } from './audit.js';

/** One place both client types report through, so the numbers are comparable. */
function recordCommand(action, outcome, ms) {
  metrics.commands.inc({ action, outcome });
  metrics.commandDuration.observe({ action }, ms);
}


const PING_INTERVAL = 20000;
const PONG_TIMEOUT = PING_INTERVAL * 4;

// Pending commands: cmdId → { resolve, reject, timer }
const pendingCommands = new Map();

/**
 * Handle a new WebSocket connection from a browser extension.
 */
export function handleConnection(ws, req) {
  // A rejected connection used to close silently, which made "it says offline"
  // impossible to diagnose from the server side.
  const from = req?.socket?.remoteAddress || 'unknown';
  let browserId = null;
  let apiKey = null;
  let persona = null;
  let authenticated = false;
  let residentialProxy = false;
  let pingTimer = null;
  let lastPong = Date.now();

  // Must authenticate within 10s
  const authTimeout = setTimeout(() => {
    if (!authenticated) {
      console.warn(`[ws] ✗ ${from} rejected: no auth message within 10s`);
      ws.close(4001, 'Auth timeout');
    }
  }, 10000);

  let authenticating = false;
  let changingControl = false;
  const localCommands = new Map();
  ws.on('message', async (raw) => {
    try {
    let msg;
    try {
      msg = JSON.parse(raw.toString());
    } catch {
      return;
    }

    if (!msg || !msg.type) return;

    // ── Auth ──
    if (msg.type === 'auth') {
      if (authenticated || authenticating) { ws.close(4003, 'Already authenticating'); return; }
      authenticating = true;
      clearTimeout(authTimeout);

      // Draining: finish what is in flight, accept nothing new, so an
      // instance can be restarted without dropping live sessions.
      if (registry.draining) {
        console.warn(`[ws] ✗ ${from} rejected: server is draining`);
        metrics.wsConnections.inc({ outcome: 'draining' });
        ws.close(4009, 'Server draining');
        return;
      }

      // An outage is retryable (generic close below); a bad or viewer credential gets 4003, which stops client reconnects.
      const presentedKey = msg.api_key;
      const principal = await authenticateToken(presentedKey, { allowBrowser: true }).catch(e => { if (e.status === 503) throw e; return null; });
      // A managed browser's credential registers only its own session.
      msg.api_key = principal?.role === 'viewer' || (principal?.role === 'browser' && principal.sessionId !== msg.browser_id) ? null : principal?.key;
      if (!msg.api_key) {
        const shown = presentedKey ? `…${String(presentedKey).slice(-4)}` : '(none sent)';
        console.warn(`[ws] ✗ ${from} rejected: unknown API key ${shown}. `
          + 'It must be listed in API_KEYS or registered for an account.');
        metrics.wsConnections.inc({ outcome: 'invalid_key' });
        ws.close(4003, 'Invalid API key');
        return;
      }

      browserId = msg.browser_id || uuidv4();
      apiKey = msg.api_key;

      // If browser_id already connected, verify the incoming key owns it
      // before kicking the existing connection. Without this check, any
      // valid key could hijack another tenant's browser_id.
      const existing = registry.get(browserId);
      if (existing && existing.apiKey !== apiKey) {
        console.warn(`[ws] ✗ ${from} rejected: browser_id ${browserId} belongs to a different key`);
        metrics.wsConnections.inc({ outcome: 'id_conflict' });
        ws.close(4003, 'browser_id registered to a different key');
        return;
      }

      authenticated = true;

      if (existing) {
        // Same owner reconnecting — replace the old socket.
        for (const [cmdId, pending] of pendingCommands) {
          if (pending.browserId === browserId) {
            clearTimeout(pending.timer);
            pendingCommands.delete(cmdId);
            recordCommand(pending.action, 'reconnected', Date.now() - pending.startedAt);
            pending.reject(new Error('Browser reconnected'));
          }
        }
        try { existing.ws.close(4000, 'Replaced by new connection'); } catch {}
        registry.remove(browserId);
        destroyMcpServer(browserId);
      }

      // A browser runs as a persona: fingerprint, cookie jar and proxy
      // together. Unspecified means this key's default, which reproduces the
      // fingerprint the key had before personas existed.
      //
      // Resolved before registering, not after: registering first passed an
      // undefined persona, so GET /browsers reported null for every browser
      // that dialled in, and a capped persona was registered before it was
      // refused.
      try {
        persona = personas.resolve(apiKey, msg.persona);
        personas.acquire(persona, browserId);
      } catch (err) {
        console.warn(`[ws] ✗ ${from} rejected: ${err.message}`);
        metrics.wsConnections.inc({ outcome: err.status === 429 ? 'persona_capped' : 'persona_unknown' });
        ws.close(4010, err.message.slice(0, 120));
        return;
      }

      // No release callback here: the socket's close handler already releases
      // the slot, and every path that removes this browser closes the socket.
      // A cloud sandbox says so at enrol time (OYA_PROVIDER in its env); the
      // dashboard's Stop needs to know, because for a cloud browser stopping
      // means destroying the sandbox, not just dropping the socket.
      const claimed = ['oya-cloud', 'oya-selfhosted', 'oya-desktop'].includes(msg.provider) ? msg.provider : 'oya-desktop';
      const provider = isProvisioned(browserId) ? 'oya-cloud' : claimed;
      const durable = await control().store.get('session', browserId);
      if (durable?.enrollmentHash && durable.enrollmentHash !== hash(String(msg.enrollment_token || ''))) throw new Error('Managed browser enrollment token required');
      await control().adopt(apiKey, { id: browserId, provider, persona: persona.id, personaLimit: persona.maxConcurrent, maxConcurrent: Number(process.env.OYA_QUOTA_MAX_BROWSERS) || 5000 });
      if (ws.readyState !== 1) { personas.release(persona, browserId); return; }
      registry.add(browserId, {
        ws, apiKey: msg.api_key, name: msg.browser_name || 'Browser', clientType: 'oya', persona, provider,
        cdp: msg.cdp === true,
      });
      registry.get(browserId).authToken = presentedKey;
      if (provider === 'oya-desktop') await keyConfig.set(apiKey, { desktop_seen_at: new Date().toISOString() });
      metrics.wsConnections.inc({ outcome: 'ok' });
      metrics.browsersConnected.set({}, registry.browsers.size);
      usage.browserConnected(apiKey, browserId);
      const fingerprint = personas.fingerprintFor(persona);

      // The proxy is part of the identity, so it travels with the fingerprint.
      const proxy = proxies.forPersona(personaOwner(apiKey), persona);
      // Only in sandboxes we run: the gateway credentials are the operator's
      // account, and a desktop the customer controls could lift them.
      const residential = provider === 'oya-cloud' && !proxy && !fingerprint.proxy?.host && proxies.residential(persona);
      if (proxy || residential) {
        // metered: the browser counts bytes through it, since the vendor bills per GB.
        fingerprint.proxy = proxy ? proxies.credentials(proxy) : { ...residential, metered: true };
        const coherent = proxies.coherence(persona, fingerprint, proxy || residential);
        if (coherent.checked && !coherent.ok) console.warn(`[proxies] ${coherent.detail}`);
      }
      residentialProxy = !!residential;

      ws.send(JSON.stringify({
        type: 'auth_ok',
        browser_id: browserId,
        control: await desktopControl(apiKey, browserId, 'get'),
        fingerprint,
        persona: { id: persona.id, name: persona.name },
        cookies: getAllCookies(persona.id),
        origins: getStorage(persona.id),
      }));

      // Send this API key's cookie jar so this browser syncs immediately.
      // Cookies are scoped per API key to prevent cross-account leakage.
      const cookies = getAllCookies(persona.id);
      if (cookies.length > 0) {
        try {
          ws.send(JSON.stringify({ type: 'cookie_sync', cookies }));
        } catch {}
      }

      // Start ping loop
      startPing();
      return;
    }

    if (!authenticated) return;
    if (msg.type === 'desktop_control') {
      if (registry.get(browserId)?.ws !== ws || typeof msg.id !== 'string' || msg.id.length > 80) return;
      if (msg.action === 'command-start' || msg.action === 'command-end') {
        try {
          if (msg.action === 'command-start') {
            if (localCommands.size >= 1024) throw new Error('Too many pending local commands');
            const finish = await control().beginCommand(browserId);
            if (ws.readyState !== 1) { await finish(); return; }
            const token = uuidv4(); localCommands.set(token, finish);
            ws.send(JSON.stringify({ type: 'desktop_control_result', id: msg.id, token }));
          } else {
            const finish = localCommands.get(msg.token);
            localCommands.delete(msg.token); await finish?.();
            ws.send(JSON.stringify({ type: 'desktop_control_result', id: msg.id }));
          }
        } catch (error) { if (ws.readyState === 1) ws.send(JSON.stringify({ type: 'desktop_control_result', id: msg.id, error: error.message })); }
        return;
      }
      if (changingControl) { ws.send(JSON.stringify({ type: 'desktop_control_result', id: msg.id, error: 'A control handoff is already in progress' })); return; }
      changingControl = true;
      try {
        const state = await desktopControl(apiKey, browserId, msg.action, () => registry.get(browserId)?.ws === ws && ws.readyState === 1);
        if (ws.readyState === 1) ws.send(JSON.stringify({ type: 'desktop_control_result', id: msg.id, state }));
      } catch (error) {
        if (ws.readyState === 1) ws.send(JSON.stringify({ type: 'desktop_control_result', id: msg.id, error: error.message, state: await desktopControl(apiKey, browserId, 'get').catch(() => null) }));
      } finally { changingControl = false; }
      return;
    }

    // ── Residential proxy bytes, counted in the sandbox (billed per GB) ──
    if (msg.type === 'proxy_bytes') {
      const bytes = Math.round(Number(msg.bytes));
      // Only sandboxes we run are given the proxy, so their count is trusted; the cap bounds a bad report.
      if (residentialProxy && registry.get(browserId)?.ws === ws && bytes > 0) usage.record(apiKey, 'residential_proxy_bytes', Math.min(bytes, 2 ** 34));
      return;
    }

    // ── CDP relayed for a gateway client (cdp-relay.js) ──
    if (msg.type === 'cdp' || msg.type === 'cdp_opened' || msg.type === 'cdp_closed') {
      if (registry.get(browserId)?.ws === ws) onRelayMessage(browserId, msg);
      return;
    }

    // ── Ping from browser — respond with pong ──
    if (msg.type === 'ping') {
      lastPong = Date.now();
      registry.updateLastSeen(browserId);
      try {
        ws.send(JSON.stringify({ type: 'pong' }));
      } catch {}
      return;
    }

    // ── Pong ──
    if (msg.type === 'pong') {
      lastPong = Date.now();
      registry.updateLastSeen(browserId);
      return;
    }

    // ── Live frame from browser ──
    if (msg.type === 'frame') {
      if (msg.data) {
        registry.pushFrame(browserId, msg.data);
        metrics.frames.inc({ client: 'oya' });
        usage.record(apiKey, 'frames');
        usage.record(apiKey, 'bytes_out', msg.data.length);
      }
      return;
    }

    // ── Cookie dump (full jar from browser on connect) ──
    //
    // Merged into this key's jar and left there. Nothing is pushed to peers:
    // fanning a full jar out to every browser in the pool on every connect was
    // O(pool size) per connect, so a fleet-wide restart was quadratic.
    if (msg.type === 'cookie_dump') {
      if (Array.isArray(msg.cookies)) {
        const merged = mergeDump(persona.id, msg.cookies);
        console.log(`[ws] Cookie dump from ${browserId}: ${msg.cookies.length} cookies, jar now ${merged.length}`);
      }
      return;
    }

    if (msg.type === 'storage_changed') {
      mergeStorage(persona.id, msg.origins);
      return;
    }

    if (msg.type === 'profile_flush') {
      drainLogins().then(() => {
        if (ws.readyState === 1) ws.send(JSON.stringify({ type: 'profile_saved', ...loginSummary(persona.id) }));
      }).catch(() => {
        if (ws.readyState === 1) ws.send(JSON.stringify({ type: 'profile_saved', error: 'Could not save profile. Try again.' }));
      });
      return;
    }

    // ── Cookie change (incremental, single or batched) ──
    //
    // Recorded in the jar only. Peers pick changes up via cookie_pull when they
    // navigate somewhere that needs them.
    if (msg.type === 'cookie_changed') {
      const changes = Array.isArray(msg.changes) ? msg.changes : (msg.change ? [msg.change] : []);
      for (const change of changes.slice(0, 500)) applyChange(persona.id, change);
      metrics.cookieChanges.inc({}, changes.length);
      return;
    }

    // ── Cookie pull (browser asks for the hosts it is about to visit) ──
    if (msg.type === 'cookie_pull') {
      const cookies = getForDomains(persona.id, msg.domains || []);
      metrics.cookiePulls.inc({});
      usage.record(apiKey, 'cookie_pulls');
      try {
        ws.send(JSON.stringify({ type: 'cookie_sync', cookies, pullId: msg.pullId }));
      } catch {}
      return;
    }

    // ── Command result ──
    if (msg.type === 'cmd_result') {
      const pending = pendingCommands.get(msg.id);
      if (pending && pending.browserId === browserId && registry.get(browserId)?.ws === ws) {
        console.log(`[ws] ← cmd_result from ${browserId}: id=${msg.id} ok=${msg.ok}`);
        clearTimeout(pending.timer);
        pendingCommands.delete(msg.id);
        metrics.pendingCommands.set({}, pendingCommands.size);
        recordCommand(pending.action, msg.ok ? 'ok' : 'error', Date.now() - pending.startedAt);
        if (pending.visible) {
          registry.recordActivity(browserId, {
            action: pending.action, summary: pending.summary, ok: !!msg.ok,
            ms: Date.now() - pending.startedAt, error: msg.error,
          });
        }
        if (msg.data?.dialog) dialogNotes.set(browserId, msg.data.dialog);
        pending.resolve({
          ok: msg.ok,
          data: msg.data,
          error: msg.error,
        });
      } else {
        console.log(`[ws] ← cmd_result from ${browserId}: id=${msg.id} (no pending command — stale or timed out)`);
      }

      // Update current URL if the result contains page info
      if (msg.data?.url) {
        registry.updateUrl(browserId, msg.data.url);
      }
      return;
    }
    } catch (err) {
      if (persona) personas.release(persona, browserId);
      console.error('[ws] registration/message rejected:', err.message);
      ws.close(4010, 'Control plane rejected connection');
    }
  });

  ws.on('close', () => {
    for (const finish of localCommands.values()) void finish().catch(() => {});
    localCommands.clear();
    clearTimeout(authTimeout);
    clearInterval(pingTimer);

    if (browserId) {
      // Guard: only clean up if WE are still the registered connection.
      // When a browser reconnects, the new connection replaces us in the
      // registry before our close event fires — removing the new entry
      // would cause the "on/off" flapping loop.
      const current = registry.get(browserId);
      if (current && current.ws === ws) {
        for (const [cmdId, pending] of pendingCommands) {
          if (pending.browserId === browserId) {
            clearTimeout(pending.timer);
            pendingCommands.delete(cmdId);
            recordCommand(pending.action, 'disconnected', Date.now() - pending.startedAt);
            pending.reject(new Error('Browser disconnected'));
          }
        }

        closeRelays(browserId);
        registry.remove(browserId);
        destroyMcpServer(browserId);
        usage.browserDisconnected(apiKey, browserId);
        personas.release(persona, browserId);
        metrics.wsDisconnections.inc({ client: 'oya' });
        metrics.browsersConnected.set({}, registry.browsers.size);
        metrics.pendingCommands.set({}, pendingCommands.size);
      }
    }
  });

  ws.on('error', () => {
    // onclose will fire after this
  });

  function startPing() {
    pingTimer = setInterval(() => {
      if (Date.now() - lastPong > PONG_TIMEOUT) {
        console.log(`[ws] Browser ${browserId} missed pongs — closing`);
        clearInterval(pingTimer);
        ws.close(4002, 'Pong timeout');
        return;
      }
      try {
        ws.send(JSON.stringify({ type: 'ping' }));
      } catch {
        clearInterval(pingTimer);
      }
    }, PING_INTERVAL);
  }
}

// ── Stream control: start/stop frame capture on the extension ──

registry.on('stream:start', ({ id }) => {
  const browser = registry.get(id);
  if (browser?.ws) {
    try {
      browser.ws.send(JSON.stringify({ type: 'stream_start', fps: 2 }));
    } catch {}
  }
});

registry.on('stream:stop', ({ id }) => {
  const browser = registry.get(id);
  if (browser?.ws) {
    try {
      browser.ws.send(JSON.stringify({ type: 'stream_stop' }));
    } catch {}
  }
});

// A dialog that fired mid-action is reported on the next tool result rather than
// thrown away. Both transports return through dispatchCommand, so stashing it
// here is the one place that covers the Oya client and cloud CDP alike.
const dialogNotes = new Map(); // browserId -> note

export function takeDialogNote(browserId) {
  const note = dialogNotes.get(browserId);
  if (note) dialogNotes.delete(browserId);
  return note || null;
}

/**
 * Send a command to a browser and wait for the result.
 * @returns {Promise<{ok: boolean, data: any, error: string?}>}
 */
export async function sendCommand(browserId, action, params = {}, timeoutMs, holder = null) {
  const finish = await control().beginCommand(browserId, holder);
  try { return await dispatchCommand(browserId, action, params, timeoutMs); }
  catch (e) {
    if (/timed out|timeout|disconnect|reconnect/i.test(e.message)) { e.code = 'command_outcome_unknown'; e.status = 504; }
    throw e;
  } finally { await finish(); }
}

function dispatchCommand(browserId, action, params = {}, timeoutMs) {
  const browser = registry.get(browserId);
  if (!browser) {
    return Promise.reject(new Error(`Browser ${browserId} not connected`));
  }

  const id = uuidv4();
  // 30s was under what a slow portal page needs: eviCore's eligibility screen answers a
  // page read in ~7s, so a type (find, click, clear, key-by-key) ran over and the agent,
  // told the type failed, typed the value a second time into a field that already had it.
  const timeout = timeoutMs || (action === 'navigate' ? 90000 : Number(process.env.OYA_COMMAND_TIMEOUT_MS) || 60000);

  // Outbound clients (CDP: Anchor, Browserbase, Steel, plain Chrome) are driven
  // directly rather than by handing a command to a socket and awaiting a
  // cmd_result. Same action vocabulary either way, so callers never branch.
  // Server-internal actions are not what the browser is "doing"; keep them
  // out of the activity log so it reads as the agent's own steps.
  // `record` is a poll, several a second while someone is recording: in the activity
  // log it would evict every action a person actually took.
  const visible = action !== 'evaluate_raw' && action !== 'record';
  const summary = summarise(action, params);
  if (visible) registry.commandStarted(browserId);

  if (browser.driver) {
    const started = Date.now();
    return browser.driver.send(action, params, timeout).then(
      (result) => {
        const ok = result?.ok !== false;
        recordCommand(action, ok ? 'ok' : 'error', Date.now() - started);
        if (visible) registry.recordActivity(browserId, { action, summary, ok, ms: Date.now() - started, error: result?.error });
        // A driven browser does not announce where it is; navigate tells us.
        if (ok && typeof result?.data?.url === 'string') registry.updateUrl(browserId, result.data.url);
        if (result?.data?.dialog) dialogNotes.set(browserId, result.data.dialog);
        return result;
      },
      (err) => {
        recordCommand(action, 'error', Date.now() - started);
        if (visible) registry.recordActivity(browserId, { action, summary, ok: false, ms: Date.now() - started, error: err.message });
        throw err;
      },
    );
  }

  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      pendingCommands.delete(id);
      metrics.pendingCommands.set({}, pendingCommands.size);
      recordCommand(action, 'timeout', timeout);
      if (visible) registry.recordActivity(browserId, { action, summary, ok: false, ms: timeout, error: 'timed out' });
      reject(new Error(`Command ${action} timed out after ${timeout / 1000}s`));
    }, timeout);

    pendingCommands.set(id, { resolve, reject, timer, browserId, action, startedAt: Date.now(), summary: visible ? summary : null, visible });
    metrics.pendingCommands.set({}, pendingCommands.size);

    console.log(`[ws] → cmd to ${browserId}: id=${id} action=${action}`);
    try {
      browser.ws.send(JSON.stringify({
        type: 'cmd',
        id,
        action,
        params,
      }));
    } catch (err) {
      clearTimeout(timer);
      pendingCommands.delete(id);
      reject(err);
    }
  });
}
