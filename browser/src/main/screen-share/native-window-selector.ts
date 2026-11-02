/** Optional stock-macOS window selection, with bounded callers and native quarantine after cancellation. */
import { createRequire } from 'node:module';
import path from 'node:path';
import { NATIVE_WINDOW_MINIMUM, NATIVE_WINDOW_TIMEOUT_MS } from './constants.ts';

/** Native source identity never crosses into the picker renderer. */
export interface NativeWindowIdentity {
  /** CGWindowID documented by Electron's native source id. */
  windowId: number;
  /** Exact owning native process, checked again before capture. */
  processId: number;
  /** Process launch timestamp prevents PID reuse from substituting a different application. */
  processStarted: number;
}
/** A candidate requires a later Oya confirmation; no adapter operation creates a stream. */
export type NativeWindowResult =
  | ({ /** Native candidate is pinned, not yet shared. */ status: 'selected' } & NativeWindowIdentity)
  | {
      /** Terminal native cancellation or a recoverable list-fallback condition. */ status:
        'cancelled' | 'unavailable' | 'failed';
    };
/** C Node-API surface is main-process-only and has no page bridge. */
export interface NativeWindowAdapter {
  /** False while a revoked native observer still awaits its terminal callback. */
  ready(): boolean;
  /** Returns admission only; callback supplies at most one terminal result. */
  start(callback: (result: unknown) => void, excluded: number[]): boolean;
  /** Requests native deactivation without pretending dismissal was acknowledged. */
  cancel(): void;
  /** Fresh native identity check before handing a matching Electron source to the site. */
  current(windowId: number, processId: number, processStarted: number): boolean;
}
/** Platform and module loading seams avoid native UI in hermetic tests. */
interface Environment {
  /** Non-macOS platforms keep their normal source list. */
  platform: string;
  /** Actual product OS version from Electron, not Darwin's kernel number. */
  version: string;
  /** A missing supported-platform artifact is unavailable, never silently a successful feature. */
  load(): NativeWindowAdapter;
}

/** Resolve the optional package from the app root, including its packaged unpacked native binary. */
const environment = (appDir: string): Environment => ({
  platform: process.platform,
  version: process.getSystemVersion?.() ?? '',
  load: () => createRequire(path.join(appDir, 'package.json'))('@oya/window-selector') as NativeWindowAdapter,
});

/** The browser still launches on older supported macOS; only direct selection has the newer requirement. */
export function nativeWindowSupported(platform: string, version: string): boolean {
  const [major, minor] = version.split('.').map(Number);
  const minimum = NATIVE_WINDOW_MINIMUM;
  return platform === 'darwin' && (major > minimum.major || (major === minimum.major && minor >= minimum.minor));
}

/** Treat malformed native metadata as failure before any source enumeration or final confirmation. */
function windowResult(value: unknown): NativeWindowResult {
  const choice = value as NativeWindowResult | null;
  if (choice?.status === 'cancelled' || choice?.status === 'failed' || choice?.status === 'unavailable') return choice;
  if (choice?.status !== 'selected') return { status: 'unavailable' };
  return validWindow(choice) ? choice : { status: 'unavailable' };
}

/** A native candidate must carry finite exact identities before it can be matched to a source. */
function validWindow(choice: NativeWindowIdentity): boolean {
  return (
    Number.isSafeInteger(choice.windowId) &&
    choice.windowId > 0 &&
    Number.isSafeInteger(choice.processId) &&
    choice.processId > 0 &&
    Number.isFinite(choice.processStarted) &&
    choice.processStarted > 0
  );
}

/** One JS caller settles independently of the native observer's safe terminal cleanup. */
class WindowSelection {
  /** One promise result even when abort, timeout and native callbacks race. */
  private readonly pending = Promise.withResolvers<NativeWindowResult>();
  /** Caller-visible result never waits indefinitely for OS cancellation acknowledgment. */
  readonly result = this.pending.promise;
  /** Native session cannot be replaced by a JS-only timeout. */
  private readonly adapter: NativeWindowAdapter;
  /** Revocation belongs to this exact Oya share request. */
  private readonly signal: AbortSignal;
  /** List fallback remains possible after a bounded abandoned picker. */
  private readonly timer: ReturnType<typeof setTimeout>;
  /** Prevent duplicate completion without discarding the native quarantine. */
  private settled = false;
  /** Install cancellation before presenting UI so immediate native answers and aborts cannot be lost. */
  constructor(adapter: NativeWindowAdapter, excluded: number[], signal: AbortSignal) {
    this.adapter = adapter;
    this.signal = signal;
    this.timer = setTimeout(() => this.stop('unavailable'), NATIVE_WINDOW_TIMEOUT_MS);
    signal.addEventListener('abort', this.cancel, { once: true });
    this.start(excluded);
  }
  /** Refused admission and synchronous native failures settle without permitting an accidental source choice. */
  private start(excluded: number[]): void {
    if (this.signal.aborted) return this.complete({ status: 'cancelled' });
    try {
      if (!this.adapter.start((result) => this.complete(windowResult(result)), excluded))
        this.complete({ status: 'unavailable' });
    } catch {
      this.stop('failed');
    }
  }
  /** The abandoned request can never be revived by a late native source result. */
  private readonly cancel = (): void => this.stop('cancelled');
  /** Keep adapter ownership quarantined while releasing the website/UI request immediately. */
  private stop(status: 'cancelled' | 'unavailable' | 'failed'): void {
    if (this.settled) return;
    try {
      this.adapter.cancel();
    } catch {
      /* Native readiness remains authoritative when deactivation fails. */
    }
    this.complete({ status });
  }
  /** Remove JS resources on every path; the bounded native observer owns any later callback. */
  private complete(result: NativeWindowResult): void {
    if (this.settled) return;
    this.settled = true;
    clearTimeout(this.timer);
    this.signal.removeEventListener('abort', this.cancel);
    this.pending.resolve(this.signal.aborted ? { status: 'cancelled' } : result);
  }
}

/** One app-level optional strategy shared by all source-picker requests. */
export class NativeWindowSelector {
  /** Host facts and loader; no website can select a package or alter this capability. */
  private readonly environment: Environment;
  /** Cache only module loading; the native adapter remains authoritative about busy/quarantined state. */
  private adapter: NativeWindowAdapter | null | undefined;
  /** Production loading is lazy so Windows/Linux and older macOS never attempt to load native code. */
  constructor(appDir: string, overrides: Partial<Environment> = {}) {
    this.environment = { ...environment(appDir), ...overrides };
  }
  /** Expose a capability state without starting selection or requesting new OS access. */
  state(): 'unsupported' | 'ready' | 'unavailable' {
    if (!nativeWindowSupported(this.environment.platform, this.environment.version)) return 'unsupported';
    try {
      return this.load()?.ready() ? 'ready' : 'unavailable';
    } catch {
      return 'unavailable';
    }
  }
  /** A broken/missing artifact retains list fallback while packaging validation fails separately. */
  private load(): NativeWindowAdapter | null {
    if (this.adapter !== undefined) return this.adapter;
    try {
      return (this.adapter = this.environment.load());
    } catch {
      return (this.adapter = null);
    }
  }
  /** The native observer refuses overlaps; no JS queue can redirect a late nil-stream result. */
  select(excluded: number[], signal: AbortSignal): Promise<NativeWindowResult> {
    if (signal.aborted) return Promise.resolve({ status: 'cancelled' });
    if (this.state() !== 'ready' || !this.adapter) return Promise.resolve({ status: 'unavailable' });
    return new WindowSelection(this.adapter, excluded, signal).result;
  }
  /** Only an exact still-live window/process identity may reach the fresh Electron source lookup. */
  current(choice: NativeWindowIdentity): boolean {
    try {
      return !!this.adapter?.current(choice.windowId, choice.processId, choice.processStarted);
    } catch {
      return false;
    }
  }
}
