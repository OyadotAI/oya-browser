/**
 * The cross-site iframes of each tab, as CDP sessions. Such an iframe runs in
 * its own process, a separate target the page's own session cannot reach, so a
 * recording has to arm it through its session. The persona applier attaches to
 * every one as the tab starts (src/anonymity/apply.ts); this keeps the list, so a
 * recording started later still finds the iframes already on the page.
 */
import type { Debugger } from 'electron';

/** Hears of one iframe session attaching. */
type Attached = (sessionId: string, frameId: string) => void;

/** Hears one CDP event's params. */
type EventListener = (params: unknown) => void;

/** A CDP target, as far as this reads it. */
interface TargetInfo {
  /** The target's id; an iframe's is its frame id. */
  targetId: string;
  /** 'iframe', 'worker', ... */
  type: string;
}

/** Someone told of each iframe that attaches. */
interface Watcher {
  /** Hears the iframe's session and frame id. */
  attached: Attached;
}

/** The part of a view's contents tracking uses. */
interface DebuggerHost {
  /** The page's debugger. */
  readonly debugger: Debugger;
}

/** A CDP target attach or detach message's params, as far as this reads them. */
interface TargetParams {
  /** The session the target attached or detached as. */
  sessionId: string;
  /** The target itself. */
  targetInfo?: TargetInfo;
}

/** A port that talks to one iframe's session. */
export interface SessionPort {
  /** Sends a command to the iframe's session. */
  send(method: string, params?: object): Promise<unknown>;
  /** Subscribes to one event from the iframe's session; returns the unsubscribe. */
  on(event: string, fn: EventListener): () => void;
}

/** One iframe session, as `list()` gives it. */
export interface FrameSession {
  /** The CDP session id. */
  sessionId: string;
  /** The iframe's target id. */
  frameId: string;
}

/** A view whose iframe sessions can be tracked; tracking leaves the list on it. */
export interface TrackedView {
  /** The page's contents; only its debugger is used. */
  readonly webContents: DebuggerHost;
  /** The view's iframe sessions, once tracked. */
  frameSessions?: FrameSessions;
}

/**
 * A view's iframe sessions. One debugger listener serves every iframe: a page
 * with dozens of them would otherwise add four listeners for each while recording.
 * It is what a recording channel needs to arm the iframes: the ones attached now,
 * a way to hear of new ones, and a port that talks to one of them.
 */
export class FrameSessions {
  /** The view's debugger, every iframe session's transport. */
  private readonly dbg: Debugger;
  /** Session id → the iframe's target id. */
  private readonly sessions = new Map<string, string>();
  /** Who is told of each iframe that attaches. */
  private readonly watchers = new Set<Watcher>();
  /** `${sessionId} ${event}` → its listeners. */
  private readonly listeners = new Map<string, Set<EventListener>>();

  /** Starts following `dbg`'s attach and detach messages and its iframes' events. */
  constructor(dbg: Debugger) {
    this.dbg = dbg;
    dbg.on('message', (_e, method, params, sessionId) => {
      this.follow(method, params);
      if (sessionId) this.dispatch(method, params, sessionId);
    });
  }

  /** Follows one attach or detach message, telling the watchers of a new iframe. */
  private follow(method: string, params: TargetParams | undefined): void {
    const info = params?.targetInfo;
    if (method === 'Target.attachedToTarget' && info?.type === 'iframe' && params) {
      this.sessions.set(params.sessionId, info.targetId);
      for (const watcher of this.watchers) watcher.attached(params.sessionId, info.targetId);
    } else if (method === 'Target.detachedFromTarget') this.sessions.delete(params?.sessionId ?? '');
  }

  /** Hands a message from an iframe's session to whoever listens for that event there. */
  private dispatch(method: string, params: unknown, sessionId: string): void {
    for (const fn of this.listeners.get(`${sessionId} ${method}`) || []) fn(params);
  }

  /** The iframes attached now. */
  list(): FrameSession[] {
    return [...this.sessions].map(([sessionId, frameId]) => ({ sessionId, frameId }));
  }

  /** Starts telling `attached` of each iframe that attaches; returns the stop. */
  watch(attached: Attached): () => boolean {
    const watcher: Watcher = { attached };
    this.watchers.add(watcher);
    return () => this.watchers.delete(watcher);
  }

  /** A port that talks to one iframe's session. */
  port(sessionId: string): SessionPort {
    return {
      send: (method, params = {}) => this.dbg.sendCommand(method, params, sessionId),
      on: (event, fn) => this.onSession(sessionId, event, fn),
    };
  }

  /** Subscribes to one CDP event from one session; returns the unsubscribe. */
  private onSession(sessionId: string, event: string, fn: EventListener): () => void {
    const key = `${sessionId} ${event}`;
    const set = this.listeners.get(key) ?? new Set<EventListener>();
    this.listeners.set(key, set.add(fn));
    return () => this.listeners.get(key)?.delete(fn);
  }
}

/** Tracks a view's iframe sessions from now on; call once per view, before its page loads. */
export function trackFrameSessions(view: TrackedView): void {
  view.frameSessions = new FrameSessions(view.webContents.debugger);
}

/** A view's iframe sessions, for a recording channel to arm; null for a view never tracked. */
export function framePorts(view: TrackedView): FrameSessions | null {
  return view.frameSessions ?? null;
}
