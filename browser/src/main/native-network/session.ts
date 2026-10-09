/** One exclusively owned private session's native request pipeline and bounded response store. */
import { requireFailureEngine } from './failures.ts';
import { randomUUID } from 'node:crypto';
import type { WebContents } from 'electron';
import { ownedFrame } from './frame-lease.ts';
import { watchRequestFrames, requireLifecycle } from './frame-lifecycle.ts';
import { NativeInterception, policyObserver } from './interception.ts';
import { NativeBodies } from './bodies.ts';
import { NativePauses } from './pauses.ts';
import { NETWORK_LIMITS } from './constants.ts';
import type {
  BodySession,
  NetworkDependencies,
  NetworkSink,
  RequestDetails,
  ResumeRequest,
  NetworkObserver,
} from './types.ts';
/** Native observer arguments stay independent from protocol transport types. */
type WatchArguments = [contents: WebContents, domain: string, emit: NetworkSink, params?: Record<string, unknown>];
/** Cleanup/update handle inputs share the same exact-page interception owner. */
type ObserverArguments = [contents: WebContents, domain: string, emit: NetworkSink, policy?: NativeInterception];
/** Only this socket's enabled native page can receive request information. */
interface Watch {
  /** Native event observers; each flat session owns its own unsubscribe. */
  network: Set<NetworkSink>;
  /** At most one interception owner per page, to avoid conflicting continuations. */
  fetch?: NativeInterception;
  /** Remove only this page’s native request-lifetime observers. */
  stopFrames(): void;
  /** Native page disposal listener, detached with the last subscription. */
  destroyed(): void;
}
/** Public identities cannot be guessed from native request sequence numbers. */
interface RequestRecord {
  /** Connection-local unguessable request id. */
  id: string;
  /** Original exact native frame identity. */
  frameId: string;
  /** Exact source page. */
  contents: WebContents;
}
/** No default-profile session is ever passed to this pipeline. */
export class NativeNetworkSession {
  /** Native session owned by one ephemeral context. */
  private readonly session: BodySession;
  /** Live authorization callbacks. */
  private readonly deps: NetworkDependencies;
  /** Page-specific subscriptions. */
  private readonly watches = new Map<WebContents, Watch>();
  /** Bounded native-to-public request registry. */
  private readonly requests = new Map<number, RequestRecord>();
  /** Native pipe bytes and waiters. */
  private readonly bodies = new NativeBodies();
  /** Native paused continuations. */
  private readonly pauses: NativePauses;
  /** Separate capture capability is enabled lazily. */
  private capture = false;
  /** Native callback installation happens once, only on a newly owned private partition. */
  constructor(session: BodySession, deps: NetworkDependencies) {
    this.session = session;
    this.deps = deps;
    this.pauses = new NativePauses(deps);
    this.install();
  }
  /** The application installs this pipeline only on a newly owned private session. */
  private install(): void {
    const requests = this.session.webRequest;
    requests.onBeforeRequest((d, cb) => this.safeBefore(d, cb));
    requests.onResponseStarted((d) => this.event('response', d));
    requests.onBeforeRedirect((d) => this.event('redirect', d));
    requests.onCompleted((d) => this.event('complete', d));
    requests.onErrorOccurred((d) => this.failed(d));
  }
  /** Every failure settles its native callback exactly once, including disposed-frame getters. */
  private safeBefore(details: RequestDetails, callback: ResumeRequest): void {
    const resume = onceRequest(callback);
    try {
      this.before(details, resume);
    } catch {
      resume({ cancel: true });
    }
  }
  /** Exact native frame-graph attribution includes cross-process children, never guessed worker parents. */
  private eligible(details: RequestDetails): WebContents | undefined {
    const contents = details.webContents;
    return contents && ownedFrame(contents, details.frame) && this.watches.has(contents) && this.deps.allowed(contents)
      ? contents
      : undefined;
  }
  /** Governance is evaluated even for requests that have no observer or interception owner. */
  private before(details: RequestDetails, resume: ResumeRequest): void {
    if (!this.deps.allowedURL(details.url)) return resume({ cancel: true });
    const contents = this.eligible(details);
    if (!contents) return resume({});
    const record = this.remember(details, contents);
    this.event('request', details);
    const policy = this.watches.get(contents)?.fetch;
    if (policy?.accepts(details)) this.pauses.hold(details, resume, policy.emit, record.id);
    else resume({});
  }
  /** Redirects keep one request identity; evictions revoke original-body readers too. */
  private remember(details: RequestDetails, contents: WebContents): RequestRecord {
    const native = details.id;
    const existing = this.requests.get(native);
    if (existing) return existing;
    if (this.requests.size >= NETWORK_LIMITS.records) this.forget(this.requests.keys().next().value!);
    const record = { id: randomUUID(), contents, frameId: this.deps.frameId(contents, details.frame!) };
    this.requests.set(native, record);
    if (this.watches.get(contents)?.network.size) this.bodies.add(native);
    return record;
  }
  /** Metadata events intentionally use Oya's schema, not fabricated inspector timing/connection fields. */
  private event(phase: string, details: RequestDetails): void {
    const record = this.requests.get(details.id);
    if (!record || !this.deps.allowed(record.contents)) return this.forget(details.id);
    const params = { ...networkEvent(record.id, phase, details), frameId: record.frameId };
    for (const emit of this.watches.get(record.contents)?.network || []) emit('Oya.networkEvent', params);
  }
  /** A native failed load cannot leave a body reader waiting for bytes that will never arrive. */
  private failed(details: RequestDetails): void {
    this.pauses.cancelRequest(details.id);
    this.bodies.finish({ id: details.id, body: '', error: details.error || 'Native request failed' });
    this.event('error', details);
  }
  /** Native capture preflight rejects an older engine instead of re-fetching through the page. */
  private enableCapture(): void {
    if (this.capture) return;
    const request = this.session.webRequest;
    if (!request._setOyaBodyListener) throw Error('Oya engine lacks native response-pipe capture');
    request._setOyaBodyListener((reply) => this.bodies.finish(reply));
    this.capture = true;
  }
  /** Each flat session's native subscription can be removed independently. */
  watch(...[contents, domain, emit, params = {}]: WatchArguments): NetworkObserver {
    const interception = domain === 'Fetch' ? new NativeInterception(emit, params) : undefined;
    requireLifecycle(contents);
    if (domain === 'OyaNetwork') this.enableCapture();
    const watch = this.pageWatch(contents);
    if (domain === 'Fetch' && watch.fetch) throw Error('Native request interception already has an owner');
    if (interception) watch.fetch = interception;
    else watch.network.add(emit);
    return this.observer(contents, domain, emit, interception);
  }
  /** Only the current exact-page owner may update interception, with current authorization rechecked. */
  private observer(...[contents, domain, emit, policy]: ObserverArguments): NetworkObserver {
    const stop = onceCleanup(() => this.unwatch(contents, domain, emit));
    if (!policy) return stop;
    return policyObserver(
      stop,
      policy,
      () => this.watches.get(contents)?.fetch === policy && this.deps.allowed(contents),
    );
  }

  /** Frame lifecycle is observed natively; destroyed pages cannot retain requests or listeners. */
  private pageWatch(contents: WebContents): Watch {
    const previous = this.watches.get(contents);
    if (previous) return previous;
    const stopFrames = this.observeFrames(contents);
    const watch: Watch = { network: new Set(), stopFrames, destroyed: () => this.stopPage(contents) };
    contents.once('destroyed', watch.destroyed);
    this.watches.set(contents, watch);
    return watch;
  }
  /** Route only native lifecycle signals to this context's continuation owner. */
  private observeFrames(contents: WebContents): () => void {
    return watchRequestFrames(contents, (process, routing) => this.pauses.changed(contents, process, routing));
  }
  /** Fetch detach cancels held requests; removing the last observer drops private retained data. */
  private unwatch(contents: WebContents, domain: string, emit: NetworkSink): void {
    const watch = this.watches.get(contents);
    if (!watch) return;
    if (domain === 'Fetch') this.dropFetch(contents, watch);
    else watch.network.delete(emit);
    if (!watch.network.size) this.clearPage(contents);
    if (!watch.fetch && !watch.network.size) this.stopPage(contents);
    this.stopCaptureIfIdle();
  }
  /** Removing the interception owner cancels its pending requests before returning. */
  private dropFetch(contents: WebContents, watch: Watch): void {
    watch.fetch = undefined;
    this.pauses.clear(contents);
  }
  /** Stop native byte copying when its last observer leaves; Fetch pauses remain independently owned. */
  private stopCaptureIfIdle(): void {
    if (!this.capture || [...this.watches.values()].some((watch) => watch.network.size)) return;
    this.session.webRequest._setOyaBodyListener!(null);
    this.capture = false;
    this.bodies.clear();
  }
  /** Only the exact initiating page can retrieve this connection's original response bytes. */
  async body(contents: WebContents, id: string): Promise<object> {
    if (!this.watches.get(contents)?.network.size) throw Error('Native response observation is disabled or revoked');
    const entry = [...this.requests].find(([, r]) => r.id === id && r.contents === contents);
    if (!entry || !this.deps.allowed(contents)) throw Error('Unknown or foreign native network request');
    const result = await this.bodies.read(entry[0]);
    if (!this.deps.allowed(contents)) throw Error('Native network read is no longer authorized');
    return result;
  }
  /** Urgent continuation never evaluates page code and rechecks current ownership inside NativePauses. */
  resolve(contents: WebContents, id: string, cancel: boolean, reason?: string): void {
    requireFailureEngine(this.session, cancel, reason);
    this.pauses.resolve(contents, id, cancel, reason);
  }
  /** Last-subscription and native-destruction cleanup share the exact same revocation path. */
  private stopPage(contents: WebContents): void {
    const watch = this.watches.get(contents);
    if (watch) contents.off('destroyed', watch.destroyed);
    watch?.stopFrames();
    this.watches.delete(contents);
    this.pauses.clear(contents);
    this.clearPage(contents);
    this.stopCaptureIfIdle();
  }
  /** Stale/disposed pages cannot retain network data. */
  private clearPage(contents: WebContents): void {
    for (const [id, record] of this.requests) if (record.contents === contents) this.forget(id);
  }
  /** Remove both native sequence mapping and any waiting response-body read. */
  private forget(id: number): void {
    this.requests.delete(id);
    this.bodies.drop(id);
  }
  /** Revoke callbacks before clearing a context's original native network state. */
  dispose(): void {
    this.pauses.clear();
    for (const contents of [...this.watches.keys()]) this.stopPage(contents);
    this.requests.clear();
    this.bodies.clear();
    if (this.capture) this.session.webRequest._setOyaBodyListener!(null);
    detachNetworkHooks(this.session);
  }
}
/** Only metadata actually supplied by the native phase is emitted. */
function networkEvent(requestId: string, phase: string, d: RequestDetails): Record<string, unknown> {
  return {
    requestId,
    phase,
    ...networkIdentity(d),
    ...networkHeaders(d),
    ...(d.redirectURL ? { redirectURL: d.redirectURL } : {}),
    ...(d.error && d.error !== 'net::OK' ? { error: d.error } : {}),
  };
}
/** Native URL and method are never inferred from the active tab or response location. */
function networkIdentity(d: RequestDetails): object {
  return { url: d.url, method: d.method, resourceType: d.resourceType };
}
/** Request/response headers exist only in their actual native phase. */
function networkHeaders(d: RequestDetails): object {
  return {
    ...(d.requestHeaders ? { requestHeaders: d.requestHeaders } : {}),
    ...(d.responseHeaders ? { responseHeaders: d.responseHeaders, status: d.statusCode } : {}),
  };
}

/** Native cancellation and exception paths consume the same callback exactly once. */
function onceRequest(callback: ResumeRequest): ResumeRequest {
  let settled = false;
  return (decision) => {
    if (settled) return;
    settled = true;
    callback(decision);
  };
}
/** Closed private contexts stay denied but no longer retain per-connection response observers. */
function detachNetworkHooks(session: BodySession): void {
  session.webRequest.onBeforeRequest((_details, cb) => cb({ cancel: true }));
  session.webRequest.onResponseStarted(null);
  session.webRequest.onBeforeRedirect(null);
  session.webRequest.onCompleted(null);
  session.webRequest.onErrorOccurred(null);
}

/** Reusing an old unsubscribe can never remove a newer interception owner. */
function onceCleanup(cleanup: () => void): () => void {
  let active = true;
  return () => {
    if (!active) return;
    active = false;
    cleanup();
  };
}
