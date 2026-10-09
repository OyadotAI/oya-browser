/** Bind native storage to authenticated persona sessions, including popup discovery and reconnect flushing. */
import type { App, Session, WebContents } from 'electron';
import type { Origins } from '../../page/login-state.ts';
import { NativeStorageSync, type NativeStoragePort } from './native-storage.ts';
import { storageImport } from './native-storage-values.ts';
import { STORAGE_FLUSH_MS, STORAGE_ORIGINS_MAX } from './constants.ts';
/** Application-owned seams: no renderer evaluation or debugging connection. */
export interface PersonaStorageDeps {
  /** Application lifecycle used to discover tabs and native popups before their navigation. */
  app: Pick<App, 'on' | 'removeListener'>;
  /** All existing surfaces, including popups, not merely the selected window. */
  contents(): WebContents[];
  /** The authenticated socket is usable only after persona initialization completes. */
  online(): boolean;
  /** Send only on this application's control socket. */
  send(message: object): boolean;
  /** Report a sanitized synchronization failure to the person. */
  report(message: string): void;
}
/** Committed top-level native navigation notification. */
type Navigation = (event: unknown, url: string) => void;
/** In-page native navigation also identifies whether it belongs to the main frame. */
type InPage = (event: unknown, url: string, main: boolean) => void;
/** One immutable persona binding; retiring it fences every asynchronous completion. */
interface Binding {
  /** The exact session identity, never a mutable current-session getter. */
  session: Session;
  /** The native observer and retained snapshots. */
  sync: NativeStorageSync;
  /** The initialization barrier awaited before authentication completes. */
  ready: Promise<void>;
  /** A newly created partition may be hydrated only before any page is exposed to it. */
  initializing: boolean;
  /** Discovery errors must not turn into a successful final flush. */
  failed: boolean;
}
/** Application lifetime coordinator; each native partition is imported at most once per process. */
export class PersonaStorage {
  /** The application's immutable seams. */
  private readonly deps: PersonaStorageDeps;
  /** Sessions exposed to any surface are never eligible for late restoration. */
  private readonly exposed = new WeakSet<Session>();
  /** Retain discovered first-party origins when switching away and back. */
  private readonly origins = new WeakMap<Session, Set<string>>();
  /** Wire a surface only once, even when a tab is moved between windows. */
  private readonly wired = new WeakSet<WebContents>();
  /** Only the authenticated persona may publish state. */
  private active: Binding | null = null;
  /** One batched delivery at a time, without repeatedly reading unchanged origins. */
  private delivering = false;
  /** A retired coordinator cannot be reactivated by a late authentication task. */
  private closed = false;
  /** Native event listeners must not retain a retired coordinator. */
  private readonly detach = new Set<() => void>();
  /** Remember exhaustion even if the affected persona is currently offline or inactive. */
  private readonly overflow = new WeakSet<Session>();
  /** Release the timer when the application shuts down. */
  private readonly timer: ReturnType<typeof setInterval>;
  /** The exact listener needed for application teardown. */
  private readonly created = (_event: unknown, contents: WebContents): void => this.attach(contents);
  /** Install before creating the shell or any page, so even pre-authentication exposure is remembered. */
  constructor(deps: PersonaStorageDeps) {
    this.deps = deps;
    deps.app.on('web-contents-created', this.created);
    for (const contents of deps.contents()) this.attach(contents);
    this.timer = setInterval(() => this.deliverPending(), STORAGE_FLUSH_MS);
    this.timer.unref();
  }
  /** Validate before retiring the previous persona; reconnect never replays imports over local logout. */
  async activate(session: Session, origins: Origins): Promise<void> {
    if (this.closed) throw Error('Native persona storage is disposed');
    storageImport(origins);
    if (this.active?.session === session) return this.active.ready;
    this.active?.sync.dispose();
    const binding = this.bind(session);
    this.active = binding;
    binding.ready = this.initialize(binding, origins);
    await binding.ready;
  }
  /** A send is permitted only while the immutable binding is still the selected authenticated persona. */
  private bind(session: Session): Binding {
    const sync: NativeStorageSync = new NativeStorageSync({
      session: session as Session & NativeStoragePort,
      ready: () => this.active?.sync === sync && this.deps.online(),
      send: (message) => this.deps.send(message),
    });
    return { session, sync, ready: Promise.resolve(), initializing: !this.exposed.has(session), failed: false };
  }
  /** Never initialize an already exposed partition, even when its current localStorage is empty. */
  private async initialize(binding: Binding, origins: Origins): Promise<void> {
    if (this.overflow.has(binding.session)) throw Error('Too many native storage origins');
    this.exposed.add(binding.session);
    await binding.sync.initialize(binding.initializing ? origins : {});
    if (this.active !== binding || binding.failed) throw Error('Native profile initialization was interrupted');
    binding.initializing = false;
    const known = new Set([...Object.keys(origins), ...(this.origins.get(binding.session) || [])]);
    for (const origin of known) await binding.sync.watch(origin);
  }
  /** Track main-frame origins only: partitioned third-party storage must not be exported as first-party data. */
  private attach(contents: WebContents): void {
    if (this.closed || contents.isDestroyed() || this.wired.has(contents)) return;
    this.wired.add(contents);
    this.exposed.add(contents.session);
    this.refuseEarlySurface(contents.session);
    this.listen(contents);
    this.discover(contents.session, contents.getURL());
  }
  /** Remove the exact callbacks on surface destruction or coordinator teardown. */
  private listen(contents: WebContents): void {
    const navigated = (_event: unknown, url: string): void => this.discover(contents.session, url);
    const inPage = (event: unknown, url: string, main: boolean): void => {
      if (main) navigated(event, url);
    };
    const detach = this.remover(contents, navigated, inPage);
    contents.on('did-navigate', navigated).on('did-navigate-in-page', inPage).once('destroyed', detach);
    this.detach.add(detach);
  }

  /** Pair native subscriptions with a single idempotent removal callback. */
  private remover(contents: WebContents, navigated: Navigation, inPage: InPage): () => void {
    const detach = (): void => {
      contents.removeListener('did-navigate', navigated).removeListener('did-navigate-in-page', inPage);
      contents.removeListener('destroyed', detach);
      this.detach.delete(detach);
    };
    return detach;
  }
  /** A programming error that exposes a hydrating partition must fail authentication, not race its pages. */
  private refuseEarlySurface(session: Session): void {
    if (this.active?.session !== session || !this.active.initializing) return;
    this.active.sync.dispose();
    this.failed(this.active);
  }
  /** Remember only bounded HTTP(S) origins, including those reached while the socket is offline. */
  private discover(session: Session, url: string): void {
    const parsed = URL.parse(url);
    if (!parsed || !['http:', 'https:'].includes(parsed.protocol)) return;
    const origins = this.origins.get(session) || new Set<string>();
    if (origins.size >= STORAGE_ORIGINS_MAX && !origins.has(parsed.origin)) return this.discoveryOverflow(session);
    origins.add(parsed.origin);
    this.origins.set(session, origins);
    const binding = this.active;
    if (binding?.session === session) this.watch(binding, parsed.origin);
  }
  /** Discovered origins wait for hydration, and a retired persona never arms another watcher. */
  private watch(binding: Binding, origin: string): void {
    void binding.ready
      .then(async () => {
        if (this.active === binding) await binding.sync.watch(origin);
      })
      .catch(() => this.failed(binding));
  }
  /** Origin exhaustion is explicit for the affected persona, rather than a silently incomplete save. */
  private discoveryOverflow(session: Session): void {
    this.overflow.add(session);
    if (this.active?.session === session) this.failed(this.active);
  }
  /** Keep a failed profile visibly unsaved; never include engine errors or credential values. */
  private failed(binding: Binding): void {
    if (this.active !== binding || binding.failed) return;
    binding.failed = true;
    this.deps.report('Native profile storage could not synchronize. Your profile has not been saved.');
  }
  /** Batch captured changes; offline snapshots stay in the native lifecycle until reconnect. */
  private deliverPending(): void {
    const binding = this.active;
    if (!binding || binding.failed || this.delivering || !this.deps.online()) return;
    this.delivering = true;
    void binding.ready
      .then(() => binding.sync.flushPending())
      .catch(() => this.failed(binding))
      .finally(() => (this.delivering = false));
  }
  /** Final/reconnect flushing re-reads native stores, rather than relying on delivery timing of mutation events. */
  async flush(): Promise<boolean> {
    const binding = this.active;
    if (!binding) return true;
    await binding.ready;
    if (binding.failed) throw Error('Native profile storage synchronization failed');
    const accepted = await binding.sync.flush();
    if (this.active !== binding) throw Error('Persona changed during profile synchronization');
    return accepted;
  }
  /** Application teardown removes discovery and prevents all pending native replies from publishing. */
  dispose(): void {
    this.closed = true;
    for (const detach of this.detach) detach();
    clearInterval(this.timer);
    this.deps.app.removeListener('web-contents-created', this.created);
    this.active?.sync.dispose();
    this.active = null;
  }
}
