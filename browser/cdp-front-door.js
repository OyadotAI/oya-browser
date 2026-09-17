/**
 * CDP front door for automation harnesses (OYA_REMOTE_DEBUGGING_PORT).
 *
 * Electron's own debug endpoint is not something an agent can use as-is: it
 * lists this browser's UI as a page an agent will happily drive, and
 * Target.createTarget answers "Not supported". This proxies Chromium's endpoint
 * (one port up, loopback only), hides the UI, and opens tabs through createTab
 * so every page an agent gets has taken the protected path — fingerprint,
 * proxy and persona partition included.
 */

const http = require('http');
const { isIP } = require('net');
const WebSocket = require('ws');

const isUi = (info) => info?.type === 'page' && /^file:.*\/renderer\/(?:index|control-shield)\.html/.test(info.url || '');

/**
 * Chromium refuses a debug request whose Host is neither an IP literal nor
 * localhost, and refuses a WebSocket upgrade carrying an Origin. Both checks
 * live on the endpoint this proxy re-issues requests to — `fetch` below sends
 * Host: 127.0.0.1, and the `ws` client sends no Origin — so they are lost
 * unless the front door makes them itself.
 *
 * Without the Host check, a page the user visits points a name it controls at
 * 127.0.0.1, reaches this port *same-origin*, reads /json/list and opens a CDP
 * socket onto tabs the persona is signed into.
 */
const localHost = (req) => {
  const raw = req.headers.host || '';
  const host = raw.startsWith('[') ? raw.slice(1, raw.indexOf(']')) : raw.split(':')[0];
  return host === 'localhost' || isIP(host) !== 0;
};

function start({ port, upstream, host, tabs, createTab, closeTab, beginCommand = () => () => {}, clientChanged = () => {}, relayToken }) {
  const up = `127.0.0.1:${upstream}`;
  const hidden = new Set();

  const refreshHidden = async () => {
    const list = await (await fetch(`http://${up}/json/list`)).json();
    for (const t of list) if (isUi(t)) hidden.add(t.id);
    return list;
  };

  // A tab's targetId, asked of its own debugger (setupTabCDP attaches it).
  const targetIdOf = async (tab) => {
    if (!tab.targetId) {
      const { targetInfo } = await tab.view.webContents.debugger.sendCommand('Target.getTargetInfo');
      tab.targetId = targetInfo.targetId;
    }
    return tab.targetId;
  };

  const openTab = async (url) => {
    const id = createTab(url || 'about:blank', true);
    const tab = tabs().find((t) => t.id === id);
    await Promise.race([tab.ready?.catch(() => {}), new Promise((r) => setTimeout(r, 15000))]);
    return targetIdOf(tab);
  };

  const server = http.createServer(async (req, res) => {
    const reqHost = req.headers.host || `127.0.0.1:${port}`;
    const rewrite = (text) => text.split(up).join(reqHost).split(`localhost:${upstream}`).join(reqHost);
    const send = (status, body) => {
      res.writeHead(status, { 'Content-Type': 'application/json' });
      res.end(typeof body === 'string' ? body : JSON.stringify(body));
    };
    if (!localHost(req)) return send(403, { error: 'Host header must be an IP address or localhost' });
    try {
      const path = req.url.split('?')[0].replace(/\/$/, '');
      if (path === '/json/new') {
        // PUT-only, as Chromium made it: a page cannot send one, so an <img> or
        // a form cannot open a tab in the persona on the victim's behalf.
        if (req.method !== 'PUT') return send(405, { error: '/json/new requires PUT' });
        const url = decodeURIComponent(req.url.split('?')[1] || '') || 'about:blank';
        const finish = await beginCommand();
        let targetId;
        try { targetId = await openTab(url); } finally { finish(); }
        const target = (await refreshHidden()).find((t) => t.id === targetId);
        return send(200, rewrite(JSON.stringify(target || { id: targetId })));
      }
      if (path === '/json' || path === '/json/list') {
        const list = await refreshHidden();
        return send(200, rewrite(JSON.stringify(list.filter((t) => !hidden.has(t.id)))));
      }
      const finish = path === '/json/version' || path === '/json/protocol' ? () => {} : await beginCommand();
      try {
        const upRes = await fetch(`http://${up}${req.url}`, { method: req.method });
        send(upRes.status, rewrite(await upRes.text()));
      } finally { finish(); }
    } catch (e) {
      send(502, { error: e.message });
    }
  });

  const wss = new WebSocket.Server({ noServer: true, perMessageDeflate: false, maxPayload: 256 * 1024 * 1024 });
  server.on('upgrade', async (req, socket, head) => {
    if (!localHost(req) || req.headers.origin) return socket.destroy();
    const m = req.url.match(/^\/devtools\/(browser|page)\/([^/?]+)/);
    try { await refreshHidden(); } catch { return socket.destroy(); }
    if (!m || hidden.has(m[2])) return socket.destroy();
    const relay = !!relayToken && req.headers['x-oya-relay'] === relayToken;
    wss.handleUpgrade(req, socket, head, (client) => bridge(client, `ws://${up}${req.url}`, m[1] === 'browser', relay));
  });

  function bridge(client, url, isBrowser, relay) {
    // Gateway relays have already acquired the server's actor-specific gate.
    // A process-random credential keeps direct clients on the local agent gate.
    if (!relay) clientChanged(1);
    const pending = new Map();
    const commandKey = msg => JSON.stringify([msg.sessionId || '', msg.id]);
    const complete = msg => { const key = commandKey(msg); pending.get(key)?.(); pending.delete(key); };
    const upstreamWs = new WebSocket(url, { perMessageDeflate: false, maxPayload: 256 * 1024 * 1024 });
    const queued = [];
    const hiddenSessions = new Set();
    const filterReplies = new Set();
    const toUpstream = (text) => (upstreamWs.readyState === WebSocket.OPEN ? upstreamWs.send(text) : queued.push(text));
    const reply = (id, result) => { complete({ id }); client.send(JSON.stringify({ id, result })); };
    const fail = (id, message, sessionId) => { complete({ id, sessionId }); client.send(JSON.stringify({ id, sessionId, error: { code: -32000, message } })); };

    upstreamWs.on('open', () => { for (const text of queued.splice(0)) upstreamWs.send(text); });
    upstreamWs.on('close', () => client.close());
    upstreamWs.on('error', () => client.close());
    client.on('close', () => { upstreamWs.close(); if (!relay) clientChanged(-1); for (const finish of pending.values()) finish(); pending.clear(); });

    let admissionQueue = Promise.resolve();
    client.on('message', data => { admissionQueue = admissionQueue.then(() => dispatch(data)).catch(() => client.close()); });
    async function dispatch(data) {
      const text = data.toString();
      let msg;
      try { msg = JSON.parse(text); } catch { return client.close(); }
      if (!Number.isFinite(msg.id) || typeof msg.method !== 'string') return client.close();
      const key = commandKey(msg);
      if (pending.has(key)) return client.close();
      pending.set(key, () => {});
      try {
        const finish = relay ? () => {} : await beginCommand();
        if (client.readyState !== WebSocket.OPEN) { finish(); return; }
        pending.set(key, finish);
      } catch (error) { return fail(msg.id, error.message, msg.sessionId); }
      if (!isBrowser) return toUpstream(text);
      if (!msg.sessionId && msg.method === 'Target.createTarget') {
        openTab(msg.params?.url).then((targetId) => reply(msg.id, { targetId }), (e) => fail(msg.id, e.message));
        return;
      }
      if (!msg.sessionId && msg.method === 'Target.closeTarget') {
        const tab = tabs().find((t) => t.targetId === msg.params?.targetId);
        if (tab) { closeTab(tab.id, { keepOne: false }); return reply(msg.id, { success: true }); }
      }
      if (!msg.sessionId && msg.method === 'Target.getTargets') filterReplies.add(msg.id);
      toUpstream(text);
    }

    upstreamWs.on('message', (data) => {
      const text = data.toString();
      try { const response = JSON.parse(text); if (response.id !== undefined) complete(response); } catch {}
      if (!isBrowser) return client.send(text);
      let msg;
      try { msg = JSON.parse(text); } catch { return client.send(text); }
      if (msg.sessionId && hiddenSessions.has(msg.sessionId)) return;
      const info = msg.params?.targetInfo;
      if (info && (hidden.has(info.targetId) || isUi(info))) {
        hidden.add(info.targetId);
        if (msg.method === 'Target.attachedToTarget') hiddenSessions.add(msg.params.sessionId);
        return;
      }
      if (filterReplies.delete(msg.id) && msg.result?.targetInfos) {
        msg.result.targetInfos = msg.result.targetInfos.filter((t) => !hidden.has(t.targetId) && !isUi(t));
        return client.send(JSON.stringify(msg));
      }
      client.send(text);
    });
  }

  server.listen(port, host, () => console.log(`[cdp] front door on ${host}:${port} → ${up}`));
  return server;
}

module.exports = { start, isUi, localHost };
