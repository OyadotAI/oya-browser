/**
 * Who drives this browser right now: a person at the desktop, or automation
 * (the server's agent, or a local CDP client). The server owns the decision
 * when connected; offline, local CDP clients and a local takeover decide it.
 * `interactive` in the snapshot is what gates human input in the shell.
 */
const { randomUUID } = require('crypto');
const {
  CONTROL_REQUEST_TIMEOUT_MS,
  LOCAL_DRAIN_TIMEOUT_MS,
  LOCAL_DRAIN_POLL_MS,
  CONTROL_TICK_MS,
  CONTROL_RENEW_BEFORE_MS,
} = require('./constants.cjs');

/** Resolves after `ms`. */
const controlPause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/** desktop_control requests to the server that are waiting for their result. */
class ControlRequests {
  /** Pending requests by id. */
  pending = new Map();

  /** `send` writes to the control socket and returns false when it is down. */
  constructor(send) {
    this.send = send;
  }

  /** Sends one desktop_control request and waits for its result. */
  request(action, extra = {}) {
    const id = randomUUID();
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => this.expire(id, reject), CONTROL_REQUEST_TIMEOUT_MS);
      this.pending.set(id, { resolve, reject, timer });
      if (!this.send({ type: 'desktop_control', id, action, ...extra })) this.unsent(id, timer, reject);
    });
  }

  /** No answer in time. */
  expire(id, reject) {
    this.pending.delete(id);
    reject(new Error('Control request timed out. Check your connection and retry.'));
  }

  /** The request never left: the socket is down. */
  unsent(id, timer, reject) {
    clearTimeout(timer);
    this.pending.delete(id);
    reject(new Error('Browser is disconnected'));
  }

  /** Removes and returns the request `id` answers, or undefined when none waits. */
  take(id) {
    const pending = this.pending.get(id);
    if (!pending) return undefined;
    clearTimeout(pending.timer);
    this.pending.delete(id);
    return pending;
  }

  /** Rejects every request still waiting on the server. */
  failAll() {
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timer);
      pending.reject(new Error('Browser disconnected during handoff'));
    }
    this.pending.clear();
  }
}

/** The control state and the handoff protocol with the server. */
class ControlState {
  /** The server's (or the local) control state: mode, mine, expiresAt, revision. */
  state = { mode: 'offline', mine: true };
  /** Whether the connected server supports desktop control. */
  supported = false;
  /** Whether the control socket is authenticated. */
  connected = false;
  /** A handoff is in progress. */
  busy = false;
  /** The handoff in progress ('acquire' or 'return'). */
  busyAction = null;
  /** A renewal request is out. */
  renewing = false;
  /** Local CDP clients connected to the front door. */
  localClients = 0;
  /** Local automation commands running now. */
  localInFlight = 0;
  /** A person took control locally, with no server involved. */
  localHeld = false;

  /** `send` writes to the control socket (false when down); `changed` hears every new snapshot. */
  constructor({ send, changed }) {
    this.requests = new ControlRequests(send);
    this.changed = changed;
    this.timer = setInterval(() => this.tick(), CONTROL_TICK_MS);
    this.timer.unref();
  }

  /** The state as the shell sees it, with whether a person may interact. */
  snapshot() {
    const { supported, connected, busy, busyAction, localClients } = this;
    const flags = { supported, connected, busy, busyAction, localClients };
    return { ...this.state, ...flags, ...this.localAgentOverride(), interactive: this.interactive() };
  }

  /** Offline, a running local command shows as agent control. */
  localAgentOverride() {
    return !this.connected && this.localInFlight && !this.localHeld ? { mode: 'agent', mine: false } : {};
  }

  /** Human input is allowed offline when no automation runs or a local takeover holds; online, when the server granted it and it has not expired. */
  interactive() {
    const { state } = this;
    if (this.busy) return false;
    if (!this.connected) {
      return (!this.localClients && !this.localInFlight) || (this.localHeld && state.mode === 'human');
    }
    return state.mode === 'human' && state.mine && state.expiresAt > Date.now();
  }

  /** Tells the listener. */
  publish() {
    this.changed(this.snapshot());
  }

  /** A state from the server; an older revision than the current one is ignored. */
  receive(value) {
    if (!value || (value.revision || 0) < (this.state.revision || 0)) return;
    this.state = value;
    this.publish();
  }

  /** Takes ('acquire') or gives back ('return') control; resolves with the new snapshot. */
  async change(action) {
    this.checkChange(action);
    this.setBusy(action);
    try {
      await this.handoff(action);
    } finally {
      this.setBusy(null);
    }
    return this.snapshot();
  }

  /** Refuses an unknown action, a second handoff, and a server without desktop control. */
  checkChange(action) {
    if (!['acquire', 'return'].includes(action)) throw new Error('Invalid control action');
    if (this.busy) throw new Error('A handoff is already in progress');
    if (this.connected && !this.supported) throw new Error('This server does not support desktop control');
  }

  /** Marks a handoff started (an action) or finished (null), and publishes. */
  setBusy(action) {
    this.busy = !!action;
    this.busyAction = action;
    this.publish();
  }

  /** The handoff itself. Admission is blocked locally before the server is asked to drain its commands. */
  async handoff(action) {
    const { requests } = this;
    let result;
    if (action === 'acquire' && !this.connected) this.holdLocally();
    if (this.connected) result = await requests.request(action === 'acquire' ? 'request' : action);
    if (action === 'acquire') result = await this.acquireAfterDrain(result);
    if (result) this.receive(result);
    else this.settleLocally(action);
  }

  /** Offline takeover: pause automation here at once. */
  holdLocally() {
    this.localHeld = true;
    this.state = { mode: 'paused', mine: true, local: true };
  }

  /** Waits for local commands to finish, then (online) asks the server to hand over. */
  async acquireAfterDrain(result) {
    const deadline = Date.now() + LOCAL_DRAIN_TIMEOUT_MS;
    while (this.localInFlight && Date.now() < deadline) await controlPause(LOCAL_DRAIN_POLL_MS);
    if (this.localInFlight) throw new Error('A local automation command is still running. Retry takeover.');
    return this.connected ? this.requests.request('acquire') : result;
  }

  /** Offline: the handoff is decided here. */
  settleLocally(action) {
    this.localHeld = action === 'acquire';
    if (!this.localHeld) this.heldAtDrop = false;
    const mode = this.localHeld ? 'human' : this.localClients ? 'agent' : 'offline';
    this.state = { mode, mine: this.localHeld, local: true };
  }

  /** Once a second: expire human control, or renew it before it runs out. */
  tick() {
    const { state } = this;
    if (!this.connected || state.mode !== 'human' || !state.mine) return;
    if (state.expiresAt <= Date.now()) {
      this.state = { mode: 'paused', mine: false };
      this.publish();
    } else if (!this.busy && !this.renewing && state.expiresAt - Date.now() < CONTROL_RENEW_BEFORE_MS) this.renew();
  }

  /** Asks the server to extend human control. */
  renew() {
    this.renewing = true;
    this.requests
      .request('renew')
      .then((value) => this.receive(value))
      .catch(() => {})
      .finally(() => (this.renewing = false));
  }
}

/** The connection and local-command half of the control state: what the socket and the front door report. */
class DesktopControl extends ControlState {
  /** A person held control when the socket dropped, so a reconnect takes it back. */
  heldAtDrop = false;

  /**
   * The socket authenticated; `value` is the server's control state, if it supports control.
   * A server that forgot the person's hold (a restart, a lapse while offline) is asked for it
   * again, so a network blip never hands a person's page to the agent mid-task.
   */
  connect(value) {
    const reclaim = this.heldAtDrop && !!value && !(value.mode === 'human' && value.mine);
    Object.assign(this, { heldAtDrop: false, connected: true, supported: !!value, localHeld: false });
    this.state = reclaim
      ? { ...value, mode: 'paused', mine: false, reclaiming: true }
      : value || { mode: 'unavailable' };
    if (reclaim) this.reclaim(value);
    else this.publish();
  }

  /** Takes control back after a reconnect; when the server refuses before answering, its own state stands. */
  reclaim(value) {
    this.change('acquire').catch(() => {
      if (this.state.reclaiming) this.state = value;
      this.publish();
    });
  }

  /** The socket closed: fall back to local control and fail every pending request. */
  disconnect() {
    const wasConnected = this.connected;
    const { localClients } = this;
    this.localHeld = localClients > 0 && (this.localHeld || this.busy || this.state.mode !== 'agent');
    this.heldAtDrop ||= wasConnected && this.state.mode === 'human' && !!this.state.mine;
    Object.assign(this, { connected: false, supported: false });
    this.state = { mode: this.disconnectedMode(wasConnected), mine: this.localHeld, local: localClients > 0 };
    this.requests.failAll();
    this.publish();
  }

  /** The mode once the socket is gone: local automation's, or plain disconnected/offline. */
  disconnectedMode(wasConnected) {
    if (this.localClients) return this.localHeld ? 'paused' : 'agent';
    return wasConnected ? 'disconnected' : 'offline';
  }

  /** A desktop_control_result. One for a request we no longer wait on still releases its command slot. */
  result(message) {
    const pending = this.requests.take(message.id);
    if (!pending) {
      if (message.token) this.requests.request('command-end', { token: message.token }).catch(() => {});
      return;
    }
    if (message.state) this.receive(message.state);
    if (message.error) pending.reject(new Error(message.error));
    else pending.resolve(message.token || message.state);
  }

  /** A local CDP client connected (+1) or left (-1). */
  localClient(delta) {
    this.localClients = Math.max(0, this.localClients + delta);
    if (!this.connected && !this.localHeld) {
      this.state = { mode: this.localClients ? 'agent' : 'offline', mine: !this.localClients };
    }
    this.publish();
  }

  /** Admits one local automation command; resolves with the function that ends it. */
  async beginLocalCommand() {
    if (this.busy || this.localHeld || (this.connected && this.state.mode !== 'agent')) {
      throw new Error('Automation paused for human control');
    }
    this.localInFlight++;
    this.publish();
    const token = this.connected ? await this.commandSlot() : undefined;
    return this.commandFinisher(token);
  }

  /** The server's command slot token; a refusal undoes the admission. */
  async commandSlot() {
    try {
      return await this.requests.request('command-start');
    } catch (error) {
      this.localInFlight--;
      this.publish();
      throw error;
    }
  }

  /** Ends a local command once, however often it is called. */
  commandFinisher(token) {
    let done = false;
    return () => {
      if (done) return;
      done = true;
      this.localInFlight--;
      if (token) this.requests.request('command-end', { token }).catch(() => {});
      this.publish();
    };
  }
}

/**
 * Whether an agent, or a person somewhere else, has the page. A lapsed hold, a
 * handoff in progress and a dropped socket are not that: nobody else is driving,
 * so what waits on the person (their recording, their sign-in popup) carries on.
 * Treating every non-interactive moment as "lost" is what stopped recordings by
 * themselves and froze Google's sign-in window.
 */
function drivenElsewhere(state) {
  return state.mode === 'agent' || (state.mode === 'human' && !state.mine);
}

/** The public methods, bound, as createControlState has always returned them. */
const CONTROL_API = [
  'snapshot',
  'change',
  'receive',
  'connect',
  'disconnect',
  'result',
  'localClient',
  'beginLocalCommand',
];

/** `send` writes to the control socket; `changed` hears every new snapshot. */
function createControlState({ send, changed }) {
  const control = new DesktopControl({ send, changed });
  const api = Object.fromEntries(CONTROL_API.map((name) => [name, control[name].bind(control)]));
  /** Stops the expiry/renewal tick. */
  api.dispose = () => clearInterval(control.timer);
  return api;
}

module.exports = { createControlState, drivenElsewhere };
