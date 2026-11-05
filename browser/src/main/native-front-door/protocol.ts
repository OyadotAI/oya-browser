/** Native CDP subset and documented Oya element tools; unknown capabilities fail explicitly. */
import { COOKIE_PARAMS } from './validation-cookies.ts';
import { NativeSubscriptions } from './subscriptions.ts';
import { NativeAutoAttach } from './auto-attach.ts';
import { NativeDiscovery } from './discovery.ts';
import { PassthroughSessions } from './passthrough.ts';
import { NativeSessions } from './sessions.ts';
import { SessionLifecycle } from './session-lifecycle.ts';
import { BrowserScopes, type BrowserScope } from './browser-scopes.ts';
import { isWebAddress, NOT_A_WEB_ADDRESS } from '../tabs/navigation.ts';
import { validateParams, nativeParameterNames } from './validation.ts';
import { NATIVE_DOOR } from './constants.ts';
import type { NativeBackend, NativeCommand, NativeTarget } from './types.ts';

/** Authenticated transport event delivery. */
type EventSink = (event: object) => void;
/** Fatal asynchronous resource failure delivery. */
type FatalSink = (reason: string) => void;
/** Non-transport callers must not silently swallow asynchronous failures. */
function fail(reason: string): never {
  throw Error(reason);
}
/** Protocol errors carry machine-readable codes without leaking backend internals. */
export class ProtocolError extends Error {
  /** JSON-RPC error category. */
  readonly code: number;
  /** Preserve the unsupported/invalid distinction. */
  constructor(message: string, code: number = NATIVE_DOOR.error) {
    super(message);
    this.code = code;
  }
}
/** Required string arguments never get coerced from objects or arrays. */
export function stringParam(params: Record<string, unknown>, name: string): string {
  if (typeof params[name] !== 'string') throw new ProtocolError(`${name} must be a string`, NATIVE_DOOR.invalid);
  return params[name];
}
/** Require a supported web address before a native tab operation. */
function address(params: Record<string, unknown>): string {
  const url = stringParam(params, 'url');
  if (!isWebAddress(url)) throw new ProtocolError(NOT_A_WEB_ADDRESS);
  return url;
}
/** Standard methods that map directly to an exact native page operation. */
const PAGE_ACTIONS: Record<string, string> = {
  'Emulation.setDeviceMetricsOverride': 'metrics:set',
  'Emulation.clearDeviceMetricsOverride': 'metrics:clear',
  'Page.getFrameTree': 'frameTree',
  'Network.getResponseBody': 'network:body',
  'Runtime.evaluate': 'runtime:evaluate',
  'Runtime.callFunctionOn': 'runtime:invoke',
  'Runtime.getProperties': 'runtime:inspect',
  'Runtime.awaitPromise': 'runtime:await',
  'Runtime.releaseObject': 'runtime:drop',
  'Runtime.releaseObjectGroup': 'runtime:dropGroup',
  'Page.createIsolatedWorld': 'runtime:createWorld',
  'Page.reload': 'page:reload',
  'Page.stopLoading': 'page:stop',
  'DOM.getDocument': 'dom:document',
  'DOM.querySelector': 'dom:query',
  'DOM.querySelectorAll': 'dom:queryAll',
  'DOM.describeNode': 'dom:describe',
  'DOM.getAttributes': 'dom:attributes',
  'DOM.getOuterHTML': 'dom:outerHTML',
  'DOM.focus': 'dom:focus',
  'DOM.scrollIntoViewIfNeeded': 'dom:scroll',
  'Input.insertText': 'input:text',
  'Input.dispatchMouseEvent': 'input:pointer',
  'Input.dispatchKeyEvent': 'input:key',
  'Oya.getNavigationHistory': 'history:read',
  'Oya.navigateToHistoryEntry': 'history:navigate',
  'Oya.analyze': 'analyze',
  'Oya.click': 'click',
  'Oya.type': 'type',
  'Oya.pressKey': 'press_key',
  'Oya.scroll': 'scroll',
  'Oya.readElements': 'read_elements',
};
/** Supported method implementations have no forwarding branch. */
const METHODS: Record<string, (session: NativeProtocol, command: NativeCommand) => unknown> = {
  'Oya.getCapabilities': () => nativeCapabilities(),
  'Storage.getCookies': (s, c) => s.manage('cookies:get', c),
  'Storage.setCookies': (s, c) => s.manage('cookies:set', c),
  'Storage.clearCookies': (s, c) => s.manage('cookies:clear', c),
  'Oya.deleteCookie': (s, c) => s.manage('cookies:delete', c),
  'Browser.setDownloadBehavior': (s, c) => (c.params.browserContextId ? s.manage('download:set', c) : {}),
  'Browser.cancelDownload': (s, c) => s.manage('download:cancel', c),
  'Target.createBrowserContext': (s, c) => s.manage('context:create', c),
  'Target.getBrowserContexts': (s, c) => s.manage('context:list', c),
  'Target.disposeBrowserContext': (s, c) => s.manage('context:dispose', c),
  'Target.setAutoAttach': (s, c) => s.scope(c).automatic.set(c.params.autoAttach === true),
  'Target.setDiscoverTargets': (s, c) => s.scope(c).discovery.set(c.params.discover === true),
  'Target.activateTarget': (s, c) => s.activate(s.target(c.params.targetId).targetId),
  'Page.bringToFront': (s, c) => s.activate(s.targetId(c)),
  'Oya.enableNetwork': (s, c) => s.observe(c, 'OyaNetwork', true),
  'Oya.disableNetwork': (s, c) => s.observe(c, 'OyaNetwork', false),
  'Fetch.enable': (s, c) => s.observe(c, 'Fetch', true),
  'Fetch.disable': (s, c) => s.observe(c, 'Fetch', false),
  'Fetch.continueRequest': (s, c) => s.resolveRequest(c),
  'Fetch.failRequest': (s, c) => s.resolveRequest(c),
  'Runtime.enable': async (s, c) => {
    await s.backend.execute(s.targetId(c), 'runtime:context', {});
    return s.observe(c, 'Runtime', true);
  },
  'Runtime.disable': (s, c) => s.observe(c, 'Runtime', false),
  'Log.enable': (s, c) => s.observe(c, 'Log', true),
  'Log.disable': (s, c) => s.observe(c, 'Log', false),
  'Page.enable': (s, c) => s.observe(c, 'Page', true),
  'Page.disable': (s, c) => s.observe(c, 'Page', false),
  'Browser.getVersion': () => ({
    protocolVersion: '1.3',
    product: 'Oya/native',
    revision: '',
    userAgent: 'Oya',
    jsVersion: '',
  }),
  'Target.getTargets': (s) => ({ targetInfos: s.backend.targets() }),
  'Target.getTargetInfo': (s, c) =>
    !c.params.targetId && s.browserSession(c) && !s.pinned
      ? { targetInfo: BROWSER_TARGET }
      : { targetInfo: s.target(c.params.targetId || s.targetId(c)) },
  'Target.createTarget': async (s, c) => ({
    targetId: await s.backend.open(
      address({ url: 'about:blank', ...c.params }),
      c.params.browserContextId as string | undefined,
    ),
  }),
  'Target.closeTarget': async (s, c) => {
    const id = s.target(c.params.targetId).targetId;
    await s.backend.close(id);
    return { success: true };
  },
  'Target.attachToTarget': (s, c) => s.attach(c),
  'Target.attachToBrowserTarget': (s, c) => s.attachBrowser(c),
  'Target.detachFromTarget': (s, c) => s.detach(c),
  'Page.navigate': async (s, c) => {
    const target = s.targetId(c);
    await s.backend.execute(target, 'navigate', { url: address(c.params) });
    return { frameId: target };
  },
  'Page.captureScreenshot': (s, c) => screenshot(s, c),
};
/** Each connection owns its session namespace; closing a target invalidates every later operation on it. */
export class NativeProtocol {
  /** Native page-only auto-attachment, separate from manual sessions. */
  readonly automatic: NativeAutoAttach;
  /** Connection-owned native target observer. */
  readonly discovery: NativeDiscovery;
  /** Application-owned capability adapter, never a CDP client. */
  readonly backend: NativeBackend;
  /** Flat sessions belong only to this socket. */
  private readonly sessions = new NativeSessions();
  /** Browser scopes share authority but never discovery or automatic attachment state. */
  private readonly scopes: BrowserScopes;
  /** Attachment revocation and backend cleanup share one resource owner. */
  private readonly lifecycle: SessionLifecycle;
  /** Event subscriptions are scoped to this socket’s exact session. */
  private readonly subscriptions = new NativeSubscriptions();
  /** Page sessions whose standard methods pass through to their tab. */
  private readonly through: PassthroughSessions;
  /** Transport event sink supplied only after the socket is authenticated. */
  private readonly emit: (event: object) => void;
  /** Optional direct page endpoint pins one target. */
  private readonly direct?: string;
  /** Initialize independent session state. */
  constructor(backend: NativeBackend, direct?: string, emit: EventSink = () => {}, fatal: FatalSink = fail) {
    this.backend = backend.connection?.() || backend;
    this.direct = direct;
    this.emit = emit;
    this.through = createPassthrough(this.backend, this.sessions, emit, fatal);
    this.scopes = this.createScopes(fatal);
    this.lifecycle = this.createLifecycle();
    ({ discovery: this.discovery, automatic: this.automatic } = this.scopes.at());
  }
  /** Each browser scope gets independent observers over the same authorized backend. */
  private createScopes(fatal: FatalSink): BrowserScopes {
    return new BrowserScopes((parent) => ({
      discovery: this.createDiscovery(parent),
      automatic: this.createAutomatic(fatal, parent),
    }));
  }
  /** Compose cleanup from the existing connection's collaborators. */
  private createLifecycle(): SessionLifecycle {
    return new SessionLifecycle({
      backend: this.backend,
      sessions: this.sessions,
      scopes: this.scopes,
      subscriptions: this.subscriptions,
      through: this.through,
      emit: this.emit,
    });
  }
  /** Discovery sees this connection's sessions, never another socket's attachment state. */
  private createDiscovery(parent?: string): NativeDiscovery {
    return new NativeDiscovery({
      backend: this.backend,
      attached: (id) => this.sessions.attached(id, parent),
      removed: (id) => this.lifecycle.removed(id),
      emit: (method, params) => this.emitScoped(method, params, parent),
    });
  }

  /** Automatic allocation shares the same session limit and emits standard flat attachment events. */
  private createAutomatic(fatal: (reason: string) => void, parent?: string): NativeAutoAttach {
    return new NativeAutoAttach({
      backend: this.backend,
      fatal,
      capacity: (n) => this.requireCapacity(n),
      attach: (target) => this.autoAttached(target, parent),
      detach: (sessionId, targetId) => this.autoDetached(sessionId, targetId, parent),
    });
  }
  /** Preflight capacity applies equally to manual and automatic sessions. */
  private requireCapacity(additional: number): void {
    this.sessions.requireCapacity(additional + this.through.size);
  }
  /** Allocate an automatic session only for a target still exposed by the native backend. */
  private autoAttached(target: NativeTarget, parent?: string): string {
    const sessionId = this.allocate(target.targetId, parent);
    const params = { sessionId, targetInfo: { ...target, attached: true }, waitingForDebugger: false };
    this.emitScoped('Target.attachedToTarget', params, parent);
    return sessionId;
  }
  /** Revoked targets may have already removed their sessions through discovery; never double-announce detach. */
  private autoDetached(sessionId: string, _targetId: string, parent?: string): void {
    if (!this.sessions.has(sessionId)) return;
    this.detach({ id: 0, method: 'Target.detachFromTarget', params: { sessionId }, sessionId: parent });
  }
  /** Release per-socket node registries and invalidate attached sessions. */
  dispose(): void {
    this.lifecycle.dispose();
  }
  /** Resolve only targets currently authorized by the native backend. */
  target(id: unknown): NativeTarget {
    return findTarget(this.backend, id);
  }
  /** Whether a page endpoint is pinned to one target. */
  get pinned(): boolean {
    return !!this.direct;
  }
  /** Whether `command` is on a page: an attached session, or the pinned page endpoint. */
  private pageSession(command: NativeCommand): boolean {
    const id = command.sessionId;
    return id ? this.sessions.has(id) && !this.sessions.browser(id) : !!this.direct;
  }
  /** Native browser commands may run only on a browser endpoint or an owned browser attachment. */
  browserSession(command: NativeCommand): boolean {
    return this.sessions.browser(command.sessionId);
  }
  /** Discovery and automatic attachment remain private to the requesting browser scope. */
  scope(command: NativeCommand): BrowserScope {
    return this.scopes.at(command.sessionId);
  }
  /** A browser attachment shares this connection's authorization without becoming a page target. */
  attachBrowser(command: NativeCommand): { /** Connection-owned browser session. */ sessionId: string } {
    this.requireCapacity(1);
    return { sessionId: this.sessions.allocate(command.sessionId).id };
  }
  /** Page commands need a valid local session or exact direct target, never the current active tab. */
  targetId(command: NativeCommand): string {
    const id = command.sessionId ? this.sessions.get(command.sessionId).target : this.direct;
    return this.target(id).targetId;
  }
  /** Only flat attachment is implemented; frame/context auto-attachment is explicitly unsupported. */
  attach(command: NativeCommand): { /** Connection-local session id. */ sessionId: string } {
    if (command.params.flatten !== true) throw new ProtocolError('Only flattened target sessions are supported');
    return { sessionId: this.allocate(command.params.targetId, command.sessionId) };
  }
  /** Allocate a session after checking exact native ownership and shared capacity. */
  private allocate(targetId: unknown, parent?: string): string {
    this.requireCapacity(1);
    const target = this.target(targetId);
    const sessionId = this.sessions.allocate(parent, target.targetId).id;
    return this.lifecycle.attached(sessionId);
  }
  /** A foreign or stale session cannot detach another connection. */
  detach(command: NativeCommand): object {
    const id = stringParam(command.params, 'sessionId');
    if (this.sessions.get(id).parent !== command.sessionId)
      throw new ProtocolError('Session belongs to another browser scope');
    this.lifecycle.close(id);
    return {};
  }
  /** Browser-level events name their requesting browser session, never an unrelated page attachment. */
  private emitScoped(method: string, params: Record<string, unknown>, parent?: string): void {
    if (parent && !this.sessions.has(parent)) return;
    this.emit({ method, params, ...(parent ? { sessionId: parent } : {}) });
  }
  /** Explicit foreground activation uses normal native tab/window ownership. */
  activate(target: string): object {
    if (!this.backend.activate) throw new ProtocolError('Native target activation is unavailable');
    this.backend.activate(target);
    return {};
  }
  /** The only out-of-band operations consume an existing native pause; no arbitrary page work. */
  resolveRequest(command: NativeCommand): object {
    if (!this.backend.resolveRequest) throw new ProtocolError('Native request control unavailable');
    this.backend.resolveRequest(
      this.targetId(command),
      stringParam(command.params, 'requestId'),
      command.method === 'Fetch.failRequest',
      command.params.errorReason as string | undefined,
    );
    return {};
  }
  /** Browser operations have no fallback and remain inside the authenticated admission gate. */
  manage(action: string, command: NativeCommand): Promise<object> {
    if (!this.backend.manage) throw new ProtocolError('Native browser management is unavailable');
    return this.backend.manage(action, command.params, (method, values) =>
      this.emitScoped(method, values, command.sessionId),
    );
  }
  /** Domains share a local session namespace but never replace each other's observers. */
  observe(command: NativeCommand, domain: string, enabled: boolean): object {
    const target = this.targetId(command),
      key = command.sessionId || target;
    const sink = (method: string, params: Record<string, unknown>): void => {
      this.emit({ method, params, ...(command.sessionId ? { sessionId: command.sessionId } : {}) });
    };
    const base = { backend: this.backend, target, key, domain, enabled };
    return this.subscriptions.set({ ...base, emit: sink, params: command.params });
  }
  /** Browser management cannot escape a pinned page or attached session. */
  private checkScope(command: NativeCommand): void {
    const page = !this.browserSession(command);
    if (page) this.targetId(command);
    const browser = isBrowserMethod(command.method);
    if (page && browser) throw new ProtocolError('Browser methods require the browser endpoint');
    if (this.direct && browser) throw new ProtocolError('Target management requires the browser endpoint');
  }
  /** Scheduling alone may inspect the same approved route; dispatch always revalidates it. */
  passesThrough(command: NativeCommand): boolean {
    return this.through.handles(command, this.pageSession(command));
  }
  /** Reject unknown commands rather than forwarding them to an engine debugging transport. */
  async dispatch(command: NativeCommand): Promise<unknown> {
    this.lifecycle.requireOpen();
    if (this.passesThrough(command)) return this.through.send(command, () => this.targetId(command));
    validateParams(command);
    this.checkScope(command);
    return dispatchNative(this, command);
  }
}
/** Target lookup never falls back to another active tab when authority has disappeared. */
function findTarget(backend: NativeBackend, id: unknown): NativeTarget {
  const target = backend.targets().find((candidate) => candidate.targetId === id);
  if (!target) throw new ProtocolError('Target is closed, unavailable, or outside this endpoint');
  return target;
}
/** Child observers share the owning connection's capacity and fatal transport cleanup. */
function createPassthrough(backend: NativeBackend, sessions: NativeSessions, emit: EventSink, fatal: FatalSink) {
  return new PassthroughSessions(backend, emit, {
    capacity: (children) => sessions.requireCapacity(children),
    fatal,
  });
}
/** Dispatch only registered native methods after scope and parameter validation. */
function dispatchNative(protocol: NativeProtocol, command: NativeCommand): unknown {
  if (Object.hasOwn(METHODS, command.method)) return METHODS[command.method](protocol, command);
  if (Object.hasOwn(PAGE_ACTIONS, command.method))
    return protocol.backend.execute(protocol.targetId(command), PAGE_ACTIONS[command.method], command.params);
  throw new ProtocolError(`Unsupported native capability: ${command.method}`, NATIVE_DOOR.unsupported);
}
/** Return only native rendered PNG/JPEG data, with no unsupported clipping silently ignored. */
async function screenshot(protocol: NativeProtocol, command: NativeCommand): Promise<object> {
  onlyParams(command, ['format', 'quality']);
  const format = command.params.format || 'png';
  if (format !== 'png' && format !== 'jpeg') throw new ProtocolError('Unsupported screenshot format');
  const data = (await protocol.backend.execute(protocol.targetId(command), 'screenshot', {
    ...command.params,
    format,
  })) as ScreenshotData;
  return { data: data.screenshot.split(',')[1] };
}
/** Native rendered capture result, never a refetched image. */
interface ScreenshotData {
  /** Native capture data URL. */
  screenshot: string;
}
/** What Target.getTargetInfo answers for the browser endpoint itself. */
const BROWSER_TARGET = { targetId: 'browser', type: 'browser', title: '', url: '', attached: true };
/** Reject semantic options this subset does not implement, especially alternate execution contexts. */
function onlyParams(command: NativeCommand, allowed: string[]): void {
  if (Object.keys(command.params).some((key) => !allowed.includes(key)))
    throw new ProtocolError('Unsupported command parameters');
}

/** One browser-scope rule serves enforcement and capability discovery, avoiding divergent documentation. */
function isBrowserMethod(method: string): boolean {
  return /^(Target|Browser)\./.test(method) || Object.hasOwn(COOKIE_PARAMS, method);
}
/** Advertise adapter support, not a claim that every engine or ownership state supports every operation. */
function nativeCapabilities(): object {
  return {
    implementation: 'oya-native',
    compatibility: 'partial',
    availability: 'checked-at-execution',
    methods: nativeMethodContracts(),
    parameterSemantics: 'Allowed names only; required values and supported subsets are validated at execution.',
    documentation: 'browser/src/main/native-front-door/README.md',
  };
}
/** Read-only discovery exposes no targets, credentials, sessions or native object handles. */
function nativeMethodContract(method: string): object {
  return {
    method,
    allowedParameters: nativeParameterNames(method),
    scope: isBrowserMethod(method) ? 'browser' : method === 'Oya.getCapabilities' ? 'connection' : 'target',
  };
}

/** Use registered dispatch keys, not a separately maintained feature checklist. */
function nativeMethodContracts(): object[] {
  return Object.keys({ ...PAGE_ACTIONS, ...METHODS })
    .sort()
    .map(nativeMethodContract);
}
