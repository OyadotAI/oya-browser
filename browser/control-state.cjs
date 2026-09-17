const { randomUUID } = require('crypto');

function createControlState({ send, changed }) {
  let state = { mode: 'offline', mine: true }, supported = false, connected = false;
  let busy = false, busyAction = null, renewing = false, localClients = 0, localInFlight = 0, localHeld = false;
  const requests = new Map();
  const snapshot = () => ({ ...state, supported, connected, busy, busyAction, localClients,
    ...(!connected && localInFlight && !localHeld ? { mode: 'agent', mine: false } : {}),
    interactive: !busy && (!connected ? (!localClients && !localInFlight || localHeld && state.mode === 'human') : state.mode === 'human' && state.mine && state.expiresAt > Date.now()) });
  const publish = () => changed(snapshot());
  const receive = value => { if (!value || (value.revision || 0) < (state.revision || 0)) return; state = value; publish(); };
  function request(action, extra = {}) {
    const id = randomUUID();
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { requests.delete(id); reject(new Error('Control request timed out. Check your connection and retry.')); }, 12000);
      requests.set(id, { resolve, reject, timer });
      if (!send({ type: 'desktop_control', id, action, ...extra })) {
        clearTimeout(timer); requests.delete(id); reject(new Error('Browser is disconnected'));
      }
    });
  }
  async function change(action) {
    if (!['acquire', 'return'].includes(action)) throw new Error('Invalid control action');
    if (busy) throw new Error('A handoff is already in progress');
    if (connected && !supported) throw new Error('This server does not support desktop control');
    busy = true; busyAction = action; publish();
    try {
      // Block admission locally before asking the server to drain its commands.
      let result;
      if (action === 'acquire' && !connected) { localHeld = true; state = { mode: 'paused', mine: true, local: true }; }
      if (connected) result = await request(action === 'acquire' ? 'request' : action);
      if (action === 'acquire') {
        const deadline = Date.now() + 10000;
        while (localInFlight && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 50));
        if (localInFlight) throw new Error('A local automation command is still running. Retry takeover.');
        if (connected) result = await request('acquire');
      }
      if (result) receive(result);
      else { localHeld = action === 'acquire'; state = { mode: localHeld ? 'human' : localClients ? 'agent' : 'offline', mine: localHeld, local: true }; }
    } finally { busy = false; busyAction = null; publish(); }
    return snapshot();
  }
  const timer = setInterval(() => {
    if (connected && state.mode === 'human' && state.mine) {
      if (state.expiresAt <= Date.now()) { state = { mode: 'paused', mine: false }; publish(); }
      else if (!busy && !renewing && state.expiresAt - Date.now() < 240000) {
        renewing = true; request('renew').then(receive).catch(() => {}).finally(() => { renewing = false; });
      }
    }
  }, 1000);
  timer.unref();
  return {
    snapshot, change, receive,
    connect(value) { connected = true; supported = !!value; localHeld = false; state = value || { mode: 'unavailable' }; publish(); },
    disconnect() {
      const wasConnected = connected;
      localHeld = localClients > 0 && (localHeld || busy || state.mode !== 'agent');
      connected = false; supported = false;
      state = { mode: localClients ? (localHeld ? 'paused' : 'agent') : wasConnected ? 'disconnected' : 'offline', mine: localHeld, local: localClients > 0 };
      for (const pending of requests.values()) { clearTimeout(pending.timer); pending.reject(new Error('Browser disconnected during handoff')); }
      requests.clear(); publish();
    },
    result(message) {
      const pending = requests.get(message.id);
      if (!pending) { if (message.token) request('command-end', { token: message.token }).catch(() => {}); return; }
      clearTimeout(pending.timer); requests.delete(message.id);
      if (message.state) receive(message.state);
      if (message.error) pending.reject(new Error(message.error)); else pending.resolve(message.token || message.state);
    },
    localClient(delta) {
      localClients = Math.max(0, localClients + delta);
      if (!connected && !localHeld) state = { mode: localClients ? 'agent' : 'offline', mine: !localClients };
      publish();
    },
    async beginLocalCommand() {
      if (busy || localHeld || connected && state.mode !== 'agent') throw new Error('Automation paused for human control');
      localInFlight++;
      publish();
      let token;
      try { if (connected) token = await request('command-start'); }
      catch (error) { localInFlight--; publish(); throw error; }
      let done = false;
      return () => { if (!done) { done = true; localInFlight--; if (token) request('command-end', { token }).catch(() => {}); publish(); } };
    },
    dispose() { clearInterval(timer); },
  };
}
module.exports = { createControlState };
