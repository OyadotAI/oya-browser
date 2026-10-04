/**
 * Who drives this browser right now: a person at the desktop, or automation
 * (the server's agent, or a local CDP client). The server owns the decision
 * when connected; offline, local CDP clients and a local takeover decide it.
 * `interactive` in the snapshot is what gates human input in the shell.
 */
import { LOCAL_DRAIN_TIMEOUT_MS, LOCAL_DRAIN_POLL_MS, CONTROL_TICK_MS, CONTROL_RENEW_BEFORE_MS } from './constants.ts';
import { ControlRequests, type SendControl } from './control-requests.ts';

export type { ControlMessage, SendControl } from './control-requests.ts';

/** The control state the server (or this app, offline) decided. */
export interface ControlValue {
  /** 'offline', 'agent', 'human', 'paused', 'disconnected' or 'unavailable'. */
  mode: string;
  /** Whether this desktop holds the control the mode names. */
  mine?: boolean;
  /** When human control lapses, in epoch milliseconds. */
  expiresAt?: number;
  /** The server's counter: an older one never replaces a newer. */
  revision?: number;
  /** Whether this app decided it, with no server involved. */
  local?: boolean;
  /** Whether a takeover is being asked for again after a reconnect. */
  reclaiming?: boolean;
}

/** The state as the shell sees it. */
export interface ControlSnapshot extends ControlValue {
  /** Whether the connected server supports desktop control. */
  supported: boolean;
  /** Whether the control socket is authenticated. */
  connected: boolean;
  /** Whether a handoff is in progress. */
  busy: boolean;
  /** The handoff in progress, or null. */
  busyAction: string | null;
  /** Local CDP clients connected to the front door. */
  localClients: number;
  /** Whether a person may use the page right now. */
  interactive: boolean;
  /** Lets the shell read it as a payload (src/shared/ipc.ts ControlState). */
  [key: string]: unknown;
}

/** A desktop_control_result from the server. */
export interface ControlResult {
  /** The request it answers. */
  id: string;
  /** A command slot's token. */
  token?: string;
  /** The control state after the request. */
  state?: ControlValue;
  /** Why the request was refused. */
  error?: string;
}

/** What a control state is built with. */
export interface ControlDeps {
  /** Writes to the control socket; false when it is down. */
  send: SendControl;
  /** Hears every new snapshot. */
  changed: (snapshot: ControlSnapshot) => void;
}

/** Resolves after `ms`. */
const controlPause = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms));

/**
 * Whether an agent, or a person somewhere else, has the page. A lapsed hold, a
 * handoff in progress and a dropped socket are not that: nobody else is driving,
 * so what waits on the person (their recording, their sign-in popup) carries on.
 * Treating every non-interactive moment as "lost" is what stopped recordings by
 * themselves and froze Google's sign-in window.
 */
export function drivenElsewhere(state: ControlValue): boolean {
  return state.mode === 'agent' || (state.mode === 'human' && !state.mine);
}

/** The control state and the handoff protocol with the server. */
class ControlState {
  /** The server's (or the local) control state: mode, mine, expiresAt, revision. */
  state: ControlValue = { mode: 'offline', mine: true };
  /** Whether the connected server supports desktop control. */
  supported = false;
  /** Whether the control socket is authenticated. */
  connected = false;
  /** A handoff is in progress. */
  busy = false;
  /** The handoff in progress ('acquire' or 'return'). */
  busyAction: string | null = null;
  /** A renewal request is out. */
  renewing = false;
  /** Local CDP clients connected to the front door. */
  localClients = 0;
  /** Local automation commands running now. */
  localInFlight = 0;
  /** A person took control locally, with no server involved. */
  localHeld = false;
  /** A person held control when the socket dropped, so a reconnect takes it back. */
  heldAtDrop = false;
  /** desktop_control requests waiting on the server. */
  protected readonly requests: ControlRequests;
  /** Hears every new snapshot. */
  private readonly changed: ControlDeps['changed'];
  /** The expiry and renewal tick. */
  private readonly timer: NodeJS.Timeout;

  /** `send` writes to the control socket (false when down); `changed` hears every new snapshot. */
  constructor({ send, changed }: ControlDeps) {
    this.requests = new ControlRequests(send);
    this.changed = changed;
    this.timer = setInterval(() => this.tick(), CONTROL_TICK_MS);
    this.timer.unref();
  }

  /** Stops the expiry/renewal tick. */
  dispose(): void {
    clearInterval(this.timer);
  }

  /** The state as the shell sees it, with whether a person may interact. */
  snapshot(): ControlSnapshot {
    const { supported, connected, busy, busyAction, localClients } = this;
    const flags = { supported, connected, busy, busyAction, localClients };
    return { ...this.state, ...flags, ...this.localAgentOverride(), interactive: this.interactive() };
  }

  /** Offline, a running local command shows as agent control. */
  private localAgentOverride(): Partial<ControlValue> {
    return !this.connected && this.localInFlight && !this.localHeld ? { mode: 'agent', mine: false } : {};
  }

  /** Human input is allowed offline when no automation runs or a local takeover holds; online, when the server granted it and it has not expired. */
  private interactive(): boolean {
    const { state } = this;
    if (this.busy) return false;
    if (!this.connected) {
      return (!this.localClients && !this.localInFlight) || (this.localHeld && state.mode === 'human');
    }
    return state.mode === 'human' && !!state.mine && (state.expiresAt ?? 0) > Date.now();
  }

  /** Tells the listener. */
  protected publish(): void {
    this.changed(this.snapshot());
  }

  /** A state from the server; an older revision than the current one is ignored. */
  receive(value: ControlValue | null | undefined): void {
    if (!value || (value.revision || 0) < (this.state.revision || 0)) return;
    this.state = value;
    this.publish();
  }

  /** Takes ('acquire') or gives back ('return') control; resolves with the new snapshot. */
  async change(action: string): Promise<ControlSnapshot> {
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
  private checkChange(action: string): void {
    if (!['acquire', 'return'].includes(action)) throw new Error('Invalid control action');
    if (this.busy) throw new Error('A handoff is already in progress');
    if (this.connected && !this.supported) throw new Error('This server does not support desktop control');
  }

  /** Marks a handoff started (an action) or finished (null), and publishes. */
  private setBusy(action: string | null): void {
    this.busy = !!action;
    this.busyAction = action;
    this.publish();
  }

  /** The handoff itself. Admission is blocked locally before the server is asked to drain its commands. */
  private async handoff(action: string): Promise<void> {
    const { requests } = this;
    let result: ControlValue | undefined;
    if (action === 'acquire' && !this.connected) this.holdLocally();
    if (this.connected) result = await requests.request<ControlValue>(action === 'acquire' ? 'request' : action);
    if (action === 'acquire') result = await this.acquireAfterDrain(result);
    if (result) this.receive(result);
    else this.settleLocally(action);
  }

  /** Offline takeover: pause automation here at once. */
  private holdLocally(): void {
    this.localHeld = true;
    this.state = { mode: 'paused', mine: true, local: true };
  }

  /** Waits for local commands to finish, then (online) asks the server to hand over. */
  private async acquireAfterDrain(result: ControlValue | undefined): Promise<ControlValue | undefined> {
    const deadline = Date.now() + LOCAL_DRAIN_TIMEOUT_MS;
    while (this.localInFlight && Date.now() < deadline) await controlPause(LOCAL_DRAIN_POLL_MS);
    if (this.localInFlight) throw new Error('A local automation command is still running. Retry takeover.');
    return this.connected ? this.requests.request<ControlValue>('acquire') : result;
  }

  /** Offline: the handoff is decided here. */
  private settleLocally(action: string): void {
    this.localHeld = action === 'acquire';
    if (!this.localHeld) this.heldAtDrop = false;
    const mode = this.localHeld ? 'human' : this.localClients ? 'agent' : 'offline';
    this.state = { mode, mine: this.localHeld, local: true };
  }

  /** Once a second: expire human control, or renew it before it runs out. */
  private tick(): void {
    const { state } = this;
    if (!this.connected || state.mode !== 'human' || !state.mine) return;
    const left = (state.expiresAt ?? NaN) - Date.now();
    if (left <= 0) {
      this.state = { mode: 'paused', mine: false };
      this.publish();
    } else if (!this.busy && !this.renewing && left < CONTROL_RENEW_BEFORE_MS) this.renew();
  }

  /** Asks the server to extend human control. */
  private renew(): void {
    this.renewing = true;
    this.requests
      .request<ControlValue>('renew')
      .then((value) => this.receive(value))
      .catch(() => {})
      .finally(() => (this.renewing = false));
  }
}

/**
 * Who drives this browser, with the connection and local-command half of the
 * control state: what the socket and the front door report.
 */
export class DesktopControl extends ControlState {
  /**
   * The socket authenticated; `value` is the server's control state, if it supports control.
   * A server that forgot the person's hold (a restart, a lapse while offline) is asked for it
   * again, so a network blip never hands a person's page to the agent mid-task.
   */
  connect(value: ControlValue | null | undefined): void {
    const reclaim = this.heldAtDrop && !!value && !(value.mode === 'human' && value.mine);
    Object.assign(this, { heldAtDrop: false, connected: true, supported: !!value, localHeld: false });
    this.state =
      reclaim && value ? { ...value, mode: 'paused', mine: false, reclaiming: true } : value || { mode: 'unavailable' };
    if (reclaim && value) this.reclaim(value);
    else this.publish();
  }

  /** Takes control back after a reconnect; when the server refuses before answering, its own state stands. */
  private reclaim(value: ControlValue): void {
    this.change('acquire').catch(() => {
      if (this.state.reclaiming) this.state = value;
      this.publish();
    });
  }

  /** The socket closed: fall back to local control and fail every pending request. */
  disconnect(): void {
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
  private disconnectedMode(wasConnected: boolean): string {
    if (this.localClients) return this.localHeld ? 'paused' : 'agent';
    return wasConnected ? 'disconnected' : 'offline';
  }

  /** A desktop_control_result. One for a request we no longer wait on still releases its command slot. */
  result(message: ControlResult): void {
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
  localClient(delta: number): void {
    this.localClients = Math.max(0, this.localClients + delta);
    if (!this.connected && !this.localHeld) {
      this.state = { mode: this.localClients ? 'agent' : 'offline', mine: !this.localClients };
    }
    this.publish();
  }

  /** Admits one local automation command; resolves with the function that ends it. */
  async beginLocalCommand(): Promise<() => void> {
    if (this.busy || this.localHeld || (this.connected && this.state.mode !== 'agent')) {
      throw new Error('Automation paused for human control');
    }
    this.localInFlight++;
    this.publish();
    const token = this.connected ? await this.commandSlot() : undefined;
    return this.commandFinisher(token);
  }

  /** The server's command slot token; a refusal undoes the admission. */
  private async commandSlot(): Promise<string> {
    try {
      return await this.requests.request<string>('command-start');
    } catch (error) {
      this.localInFlight--;
      this.publish();
      throw error;
    }
  }

  /** Ends a local command once, however often it is called. */
  private commandFinisher(token: string | undefined): () => void {
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
