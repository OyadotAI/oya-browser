/**
 * One harness WebSocket bridged to Chromium's debugger. Each command is
 * admitted through the control state (so a person's takeover pauses it), and
 * on the browser endpoint the UI and out-of-run targets are hidden and tabs
 * are opened and closed the app's way.
 */
import { WebSocket } from 'ws';
import { FRONT_DOOR_MAX_PAYLOAD, CDP_SERVER_ERROR } from './constants.ts';
import { answerFor } from './guard.ts';
import type { FrontDoor } from './door.ts';
import type { CdpCommand, CdpMessage, CdpResult, Finish, TargetInfo } from './types.ts';

/** Browser-level methods a validation run may send; everything else is refused. */
const RUN_BROWSER_METHODS = [
  'Browser.getVersion',
  'Target.setAutoAttach',
  'Target.setDiscoverTargets',
  'Target.getTargets',
  'Target.getTargetInfo',
  'Target.attachToTarget',
  'Target.detachFromTarget',
  'Target.createTarget',
  'Target.closeTarget',
];

/** A browser-endpoint command the front door may answer itself: true when handled, false to forward. */
type BrowserCommand = (bridge: Bridge, msg: CdpCommand) => boolean;

/** Parses JSON; null when it is not an object. */
const parseMessage = (text: string): CdpMessage | null => {
  try {
    return (JSON.parse(text) as CdpMessage | null) ?? null;
  } catch {
    return null;
  }
};

/** Browser-endpoint commands the front door answers itself; true when handled, false to forward. */
const BROWSER_COMMANDS: Record<string, BrowserCommand> = {
  /** A new tab goes through the app's createTab. */
  'Target.createTarget': (bridge, msg) => {
    bridge.door.openTab(msg.params?.url).then(
      (targetId) => bridge.reply(msg.id, { targetId }, msg.sessionId),
      (e: Error) => bridge.fail(msg.id, e.message, msg.sessionId),
    );
    return true;
  },
  /** Closing one of the app's tabs goes through the app, which never recreates it. */
  'Target.closeTarget': (bridge, msg) => {
    const tab = bridge.door.options.tabs().find((t) => t.targetId === msg.params?.targetId);
    if (!tab) return false;
    bridge.door.options.closeTab(tab.id, { keepOne: false });
    bridge.reply(msg.id, { success: true }, msg.sessionId);
    return true;
  },
  /** The reply is filtered on the way back. */
  'Target.getTargets': (bridge, msg) => {
    bridge.filterReplies.add(msg.id);
    return false;
  },
  /**
   * A second browser session (Playwright's newCDPSession opens one). Forwarded,
   * and the session it answers with is remembered, so its commands get what
   * the browser endpoint's own do: hidden UI, tabs the app's way.
   */
  'Target.attachToBrowserTarget': (bridge, msg) => {
    bridge.browserAttaches.add(msg.id);
    return false;
  },
};

/** Whether a command speaks for the whole browser: on the browser endpoint itself, or a session attached to it. */
const atBrowserLevel = (bridge: Bridge, msg: CdpMessage): boolean =>
  !msg.sessionId || bridge.browserSessions.has(msg.sessionId);

/** Remembers the session a Target.attachToBrowserTarget reply names. */
function noteBrowserSession(bridge: Bridge, msg: CdpMessage): void {
  if (msg.id === undefined || !bridge.browserAttaches.delete(msg.id) || !msg.result?.sessionId) return;
  bridge.browserSessions.add(msg.result.sessionId);
}

/**
 * Whether an attached target is a frame or worker of a run tab: it attaches
 * under that tab's session. Hiding it would leave it paused for a debugger
 * that never comes, so its page never loads.
 */
function joinsRun(msg: CdpMessage, info: TargetInfo, hiddenSessions: Set<string>): boolean {
  const underRunTab = !!msg.sessionId && !hiddenSessions.has(msg.sessionId);
  return msg.method === 'Target.attachedToTarget' && underRunTab && info.type !== 'page';
}

/**
 * Whether the guard answered the command itself, before it takes an admission
 * slot: refused with its reason, or answered without reaching Chromium.
 */
function answeredByGuard(bridge: Bridge, msg: CdpCommand): boolean {
  const answer = answerFor(msg, bridge.relay);
  if (!answer) return false;
  if (answer.error !== undefined) bridge.fail(msg.id, answer.error, msg.sessionId);
  else bridge.reply(msg.id, answer.result, msg.sessionId);
  return true;
}

/** Whether a command reaches a session or target outside the validation run. */
function outsideRun(bridge: Bridge, msg: CdpCommand): boolean {
  if (msg.sessionId && bridge.hiddenSessions.has(msg.sessionId)) return true;
  return !!msg.params?.targetId && !bridge.door.inRun(msg.params.targetId);
}

/** In a validation run: the answer that refuses (or stubs) a command, or null to let it through. */
function runRefusal(bridge: Bridge, msg: CdpCommand): (() => void) | null {
  if (outsideRun(bridge, msg)) return () => bridge.fail(msg.id, 'Target is outside this validation', msg.sessionId);
  if (msg.sessionId) return null;
  if (msg.method === 'Browser.setDownloadBehavior') return () => bridge.reply(msg.id, {});
  if (!RUN_BROWSER_METHODS.includes(msg.method)) {
    return () => bridge.fail(msg.id, 'Browser command unavailable for validation');
  }
  return null;
}

/** The parsed command, or null when the text is not one (garbage, or no numeric id and string method). */
function asCommand(text: string): CdpCommand | null {
  const msg = parseMessage(text);
  const valid = msg && Number.isFinite(msg.id) && typeof msg.method === 'string';
  return valid ? (msg as CdpCommand) : null;
}

/** One harness connection and its upstream socket. */
export class Bridge {
  /** Commands admitted and awaiting Chromium's reply, by [sessionId, id], with the function that ends each. */
  readonly pending = new Map<string, Finish>();
  /** Messages sent before the upstream socket opened. */
  readonly queued: string[] = [];
  /** Sessions attached to hidden targets: their traffic never reaches the harness. */
  readonly hiddenSessions = new Set<string>();
  /** Target.getTargets ids whose replies are filtered. */
  readonly filterReplies = new Set<number>();
  /** Target.attachToBrowserTarget ids awaiting the session they open. */
  readonly browserAttaches = new Set<number>();
  /** Sessions attached to the browser itself, which speak for it as the browser endpoint does. */
  readonly browserSessions = new Set<string>();
  /** Serializes admission so commands keep their order. */
  admissionQueue: Promise<void> = Promise.resolve();
  /** Ids of commands the bridge sends itself; their replies never reach the harness. */
  readonly ownIds = new Set<number>();
  /** The next such id: negative, so it never meets a harness's. */
  nextOwnId = -1;
  /** The door this bridge serves. */
  readonly door: FrontDoor;
  /** The harness's socket. */
  readonly client: WebSocket;
  /** Whether this is the browser endpoint, not a page's. */
  readonly isBrowser: boolean;
  /** Whether the harness is the server's relay, already admitted there. */
  readonly relay: boolean;
  /** Chromium's socket for this harness. */
  readonly upstream: WebSocket;

  /** Bridges `client` to Chromium at `url`; `relay` marks the server's gateway, already admitted there. */
  constructor(door: FrontDoor, client: WebSocket, url: string, isBrowser: boolean, relay: boolean) {
    this.door = door;
    this.client = client;
    this.isBrowser = isBrowser;
    this.relay = relay;
    // Gateway relays have already acquired the server's actor-specific gate.
    // A process-random credential keeps direct clients on the local agent gate.
    if (!relay) door.options.clientChanged(1);
    this.upstream = new WebSocket(url, { perMessageDeflate: false, maxPayload: FRONT_DOOR_MAX_PAYLOAD });
    this.wireUpstream();
    this.wireClient();
  }

  /** Upstream lifecycle and replies. */
  private wireUpstream(): void {
    const { upstream, client } = this;
    upstream.on('open', () => {
      for (const text of this.queued.splice(0)) upstream.send(text);
    });
    upstream.on('close', () => client.close());
    upstream.on('error', () => client.close());
    upstream.on('message', (data: Buffer) => this.fromUpstream(data.toString()));
  }

  /** Harness lifecycle and commands. */
  private wireClient(): void {
    this.client.on('close', () => this.clientClosed());
    this.client.on('message', (data: Buffer) => {
      this.admissionQueue = this.admissionQueue.then(() => this.dispatch(data)).catch(() => this.client.close());
    });
  }

  /** The harness left: close upstream and end every admitted command. */
  private clientClosed(): void {
    this.upstream.close();
    if (!this.relay) this.door.options.clientChanged(-1);
    for (const finish of this.pending.values()) finish();
    this.pending.clear();
  }

  /** The pending-map key of a command or its reply. */
  private commandKey(msg: CdpMessage): string {
    return JSON.stringify([msg.sessionId || '', msg.id]);
  }

  /** A command was answered: end its admission. */
  private complete(msg: CdpMessage): void {
    const key = this.commandKey(msg);
    this.pending.get(key)?.();
    this.pending.delete(key);
  }

  /** Sends to Chromium, or queues until the socket opens. */
  private toUpstream(text: string): void {
    if (this.upstream.readyState === WebSocket.OPEN) this.upstream.send(text);
    else this.queued.push(text);
  }

  /** Answers a command ourselves, on the session it came on. */
  reply(id: number, result: CdpResult, sessionId?: string): void {
    this.complete({ id, sessionId });
    this.client.send(JSON.stringify({ id, sessionId, result }));
  }

  /** Refuses a command with a CDP error. */
  fail(id: number, message: string, sessionId?: string): void {
    this.complete({ id, sessionId });
    this.client.send(JSON.stringify({ id, sessionId, error: { code: CDP_SERVER_ERROR, message } }));
  }

  /** One harness message: validated, held to the guard, admitted, then answered here or forwarded. */
  private async dispatch(data: Buffer): Promise<void> {
    const text = data.toString();
    const msg = this.accept(text);
    if (!msg || answeredByGuard(this, msg)) return;
    if (!(await this.admit(msg, this.commandKey(msg)))) return;
    const refusal = this.door.options.runToken && runRefusal(this, msg);
    if (refusal) return refusal();
    this.forward(msg, text);
  }

  /** The parsed command, or null after closing a harness that sent garbage or reused a pending id. */
  private accept(text: string): CdpCommand | null {
    const msg = asCommand(text);
    if (!msg || this.pending.has(this.commandKey(msg))) {
      this.client.close();
      return null;
    }
    return msg;
  }

  /** Holds the command's slot; false when it was refused or the harness left meanwhile. */
  private async admit(msg: CdpCommand, key: string): Promise<boolean> {
    this.pending.set(key, () => {});
    const finish = await this.commandSlot().catch((error: Error) => this.fail(msg.id, error.message, msg.sessionId));
    if (!finish) return false;
    if (this.client.readyState !== WebSocket.OPEN) return (finish(), false);
    this.pending.set(key, finish);
    return true;
  }

  /** The function that ends this command's admission; a relay was admitted by the server already. */
  private commandSlot(): Promise<Finish> {
    return Promise.resolve().then(() => (this.relay ? () => {} : this.door.options.beginCommand()));
  }

  /** Forwards a command, unless the browser endpoint answers it here. */
  private forward(msg: CdpCommand, text: string): void {
    if (!this.isBrowser) return this.toUpstream(text);
    const own = atBrowserLevel(this, msg) && Object.hasOwn(BROWSER_COMMANDS, msg.method);
    if (own && BROWSER_COMMANDS[msg.method](this, msg)) return;
    this.toUpstream(text);
  }

  /** One message from Chromium: ends its command's admission, then reaches the harness unless hidden. */
  private fromUpstream(text: string): void {
    const msg = parseMessage(text);
    if (msg?.id !== undefined && this.ownIds.delete(msg.id)) return;
    if (msg?.id !== undefined) this.complete(msg);
    if (!this.isBrowser || !msg) return this.client.send(text);
    noteBrowserSession(this, msg);
    if (msg.sessionId && this.hiddenSessions.has(msg.sessionId)) return;
    if (this.hideTarget(msg) || this.sendFilteredTargets(msg)) return;
    this.client.send(text);
  }

  /** A Target.getTargets reply goes back without the hidden targets; true when this was one. */
  private sendFilteredTargets(msg: CdpMessage): boolean {
    if (msg.id === undefined || !this.filterReplies.delete(msg.id) || !msg.result?.targetInfos) return false;
    msg.result.targetInfos = msg.result.targetInfos.filter((t) => !this.door.isHidden(t));
    this.client.send(JSON.stringify(msg));
    return true;
  }

  /** A target event about a hidden target: remember it (and its session) and swallow the event. */
  private hideTarget(msg: CdpMessage): boolean {
    const info = msg.params?.targetInfo;
    if (!info) return false;
    if (joinsRun(msg, info, this.hiddenSessions)) this.door.runChildren.add(info.targetId ?? '');
    if (!this.door.isHidden(info)) return false;
    this.door.hidden.add(info.targetId ?? '');
    if (msg.method === 'Target.attachedToTarget') this.release(msg);
    return true;
  }

  /**
   * A hidden target was attached, often paused until a debugger resumes it: the
   * harness never sees it, so the bridge resumes it and lets it go.
   */
  private release(msg: CdpMessage): void {
    const child = msg.params?.sessionId ?? '';
    this.hiddenSessions.add(child);
    this.sendOwn({ method: 'Runtime.runIfWaitingForDebugger', sessionId: child });
    this.sendOwn({ method: 'Target.detachFromTarget', params: { sessionId: child }, sessionId: msg.sessionId });
  }

  /** Sends a command of the bridge's own; its reply is dropped. */
  private sendOwn(command: CdpMessage): void {
    const id = this.nextOwnId--;
    this.ownIds.add(id);
    this.toUpstream(JSON.stringify({ id, ...command }));
  }
}
