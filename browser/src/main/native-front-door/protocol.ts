/** Native CDP subset and documented Oya element tools; unknown capabilities fail explicitly. */
import { COOKIE_PARAMS } from './validation-cookies.ts';
import { NativeSubscriptions } from './subscriptions.ts';
import { NativeAutoAttach } from './auto-attach.ts';
import { NativeDiscovery } from './discovery.ts';
import { randomUUID } from 'node:crypto';
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
  'Storage.getCookies': (s, c) => s.manage('cookies:get', c.params),
  'Storage.setCookies': (s, c) => s.manage('cookies:set', c.params),
  'Storage.clearCookies': (s, c) => s.manage('cookies:clear', c.params),
  'Oya.deleteCookie': (s, c) => s.manage('cookies:delete', c.params),
  'Browser.setDownloadBehavior': (s, c) => s.manage('download:set', c.params),
  'Browser.cancelDownload': (s, c) => s.manage('download:cancel', c.params),
  'Target.createBrowserContext': (s, c) => s.manage('context:create', c.params),
  'Target.getBrowserContexts': (s, c) => s.manage('context:list', c.params),
  'Target.disposeBrowserContext': (s, c) => s.manage('context:dispose', c.params),
  'Target.setAutoAttach': (s, c) => s.automatic.set(c.params.autoAttach === true),
  'Target.setDiscoverTargets': (s, c) => s.discovery.set(c.params.discover === true),
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
  'Target.getTargetInfo': (s, c) => ({ targetInfo: s.target(c.params.targetId || s.targetId(c)) }),
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
  'Target.detachFromTarget': (s, c) => s.detach(c),
  'Page.navigate': async (s, c) => {
    const target = s.targetId(c);
    await s.backend.execute(target, 'navigate', { url: address(c.params) });
    return { frameId: target };
  },
  'Page.captureScreenshot': (s, c) => s.screenshot(c),
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
  private readonly sessions = new Map<string, string>();
  /** Event subscriptions are scoped to this socket’s exact session. */
  private readonly subscriptions = new NativeSubscriptions();
  /** Transport event sink supplied only after the socket is authenticated. */
  private readonly emit: (event: object) => void;
  /** Optional direct page endpoint pins one target. */
  private readonly direct?: string;
  /** Initialize independent session state. */
  constructor(backend: NativeBackend, direct?: string, emit: EventSink = () => {}, fatal: FatalSink = fail) {
    this.backend = backend.connection?.() || backend;
    this.direct = direct;
    this.emit = emit;
    this.discovery = this.createDiscovery();
    this.automatic = this.createAutomatic(fatal);
  }
  /** Discovery sees this connection's sessions, never another socket's attachment state. */
  private createDiscovery(): NativeDiscovery {
    return new NativeDiscovery({
      backend: this.backend,
      attached: (id) => [...this.sessions.values()].includes(id),
      removed: (id) => this.targetRemoved(id),
      emit: (method, params) => this.emit({ method, params }),
    });
  }

  /** Automatic allocation shares the same session limit and emits standard flat attachment events. */
  private createAutomatic(fatal: (reason: string) => void): NativeAutoAttach {
    return new NativeAutoAttach({
      backend: this.backend,
      fatal,
      capacity: (n) => this.requireCapacity(n),
      attach: (target) => this.autoAttached(target),
      detach: (sessionId, targetId) => this.autoDetached(sessionId, targetId),
    });
  }
  /** Preflight capacity applies equally to manual and automatic sessions. */
  private requireCapacity(additional: number): void {
    if (this.sessions.size + additional > NATIVE_DOOR.maxSessions)
      throw new ProtocolError('Native session limit exceeded');
  }
  /** Allocate an automatic session only for a target still exposed by the native backend. */
  private autoAttached(target: NativeTarget): string {
    const sessionId = this.allocate(target.targetId);
    this.emit({
      method: 'Target.attachedToTarget',
      params: { sessionId, targetInfo: { ...target, attached: true }, waitingForDebugger: false },
    });
    return sessionId;
  }
  /** Revoked targets may have already removed their sessions through discovery; never double-announce detach. */
  private autoDetached(sessionId: string, targetId: string): void {
    if (!this.sessions.has(sessionId)) return;
    this.detach({ id: 0, method: 'Target.detachFromTarget', params: { sessionId } });
    this.emit({ method: 'Target.detachedFromTarget', params: { sessionId, targetId } });
  }
  /** Release per-socket node registries and invalidate attached sessions. */
  dispose(): void {
    this.discovery.dispose();
    this.automatic.dispose();
    this.subscriptions.dispose();
    this.sessions.clear();
    this.backend.dispose?.();
  }
  /** Resolve only targets currently authorized by the native backend. */
  target(id: unknown): NativeTarget {
    const target = this.backend.targets().find((candidate) => candidate.targetId === id);
    if (!target) throw new ProtocolError('Target is closed, unavailable, or outside this endpoint');
    return target;
  }
  /** Page commands need a valid local session or exact direct target, never the current active tab. */
  targetId(command: NativeCommand): string {
    const id = command.sessionId ? this.sessions.get(command.sessionId) : this.direct;
    return this.target(id).targetId;
  }
  /** Only flat attachment is implemented; frame/context auto-attachment is explicitly unsupported. */
  attach(command: NativeCommand): { /** Connection-local session id. */ sessionId: string } {
    if (this.sessions.size >= NATIVE_DOOR.maxSessions) throw new ProtocolError('Native session limit exceeded');
    if (command.params.flatten !== true) throw new ProtocolError('Only flattened target sessions are supported');
    return { sessionId: this.allocate(command.params.targetId) };
  }
  /** Allocate a session after checking exact native ownership and shared capacity. */
  private allocate(targetId: unknown): string {
    this.requireCapacity(1);
    const target = this.target(targetId);
    const sessionId = randomUUID();
    this.sessions.set(sessionId, target.targetId);
    this.discovery.refresh();
    return sessionId;
  }
  /** A foreign or stale session cannot detach another connection. */
  detach(command: NativeCommand): object {
    const id = stringParam(command.params, 'sessionId');
    this.subscriptions.release(id);
    if (!this.sessions.delete(id)) throw new ProtocolError('Unknown session');
    this.automatic.forget(id);
    this.discovery.refresh();
    return {};
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
  manage(action: string, params: Record<string, unknown>): Promise<object> {
    if (!this.backend.manage) throw new ProtocolError('Native browser management is unavailable');
    return this.backend.manage(action, params, (method, values) => this.emit({ method, params: values }));
  }
  /** Native target removal invalidates only this connection's matching sessions and event listeners. */
  private targetRemoved(targetId: string): void {
    for (const [sessionId, id] of this.sessions) {
      if (id !== targetId) continue;
      this.subscriptions.release(sessionId);
      this.sessions.delete(sessionId);
      this.emit({ method: 'Target.detachedFromTarget', params: { sessionId, targetId } });
    }
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
  /** Return only native rendered PNG/JPEG data, with no unsupported clipping silently ignored. */
  async screenshot(command: NativeCommand): Promise<object> {
    onlyParams(command, ['format']);
    const format = command.params.format || 'png';
    if (format !== 'png' && format !== 'jpeg') throw new ProtocolError('Unsupported screenshot format');
    const data = (await this.backend.execute(this.targetId(command), 'screenshot', { format })) as {
      /** Native capture data URL. */
      screenshot: string;
    };
    return { data: data.screenshot.split(',')[1] };
  }
  /** Browser management cannot escape a pinned page or attached session. */
  private checkScope(command: NativeCommand): void {
    if (command.sessionId) this.targetId(command);
    const browser = isBrowserMethod(command.method);
    if (command.sessionId && browser) throw new ProtocolError('Browser methods require the browser endpoint');
    if (this.direct && browser) throw new ProtocolError('Target management requires the browser endpoint');
  }
  /** Reject unknown commands rather than forwarding them to an engine debugging transport. */
  async dispatch(command: NativeCommand): Promise<unknown> {
    validateParams(command);
    this.checkScope(command);
    if (Object.hasOwn(METHODS, command.method)) return METHODS[command.method](this, command);
    if (Object.hasOwn(PAGE_ACTIONS, command.method))
      return this.backend.execute(this.targetId(command), PAGE_ACTIONS[command.method], command.params);
    throw new ProtocolError(`Unsupported native capability: ${command.method}`, NATIVE_DOOR.unsupported);
  }
}
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
