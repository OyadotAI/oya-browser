/** Fail-closed main ownership gates for the explicitly selected experimental native browser-passkey protocol. */
import type { Session, SelectWebauthnAccountDetails, WebContents } from 'electron';
import { isIncognito } from '../incognito/index.ts';
import { desktopRuntime } from './runtime.ts';
import { passkeySource, type PasskeySource } from './passkey-request.ts';
import { BrowserPasskeyLease } from './browser-passkey-lease.ts';
import { BROWSER_PASSKEY_REQUEST_ID_LIMIT, BROWSER_PASSKEY_TRANSPORTS } from './constants.ts';
import type {
  BrowserPasskeyCancel,
  BrowserPasskeyCall,
  BrowserPasskeyDeps,
  BrowserPasskeyDetails,
  BrowserPasskeyEnded,
  BrowserPasskeyEvents,
} from './browser-passkey-types.ts';

/** Native metadata is checked without consulting or accepting any page-supplied authority. */
function validDetails(details: BrowserPasskeyDetails): boolean {
  return (
    !!details &&
    validAttempt(details) &&
    typeof details.relyingPartyId === 'string' &&
    !!details.relyingPartyId &&
    validTransports(details.transports)
  );
}
/** Only fixed modal assertion metadata can allocate an application-wide native lease. */
function validAttempt(details: BrowserPasskeyDetails): boolean {
  return (
    typeof details.requestId === 'string' &&
    !!details.requestId &&
    details.requestId.length <= BROWSER_PASSKEY_REQUEST_ID_LIMIT &&
    ['start', 'complete'].includes(details.phase) &&
    details.kind === 'get' &&
    details.presentation === 'modal'
  );
}
/** Native transport categories remain bounded and cannot carry credential IDs or arbitrary payloads. */
function validTransports(transports: unknown): boolean {
  return (
    Array.isArray(transports) &&
    transports.length <= BROWSER_PASSKEY_TRANSPORTS.length &&
    transports.every((value) => BROWSER_PASSKEY_TRANSPORTS.some((known) => known === value))
  );
}
/** Explicit personal desktop consent never grants a governed, incognito, recording or another-profile session access. */
function personal(deps: BrowserPasskeyDeps, contents: WebContents): boolean {
  return (
    desktopRuntime(deps.config) &&
    !deps.recorder.recording &&
    !isIncognito(contents.session) &&
    contents.session === deps.persona.session()
  );
}
/** Use actual native document identity; metadata cannot replace a committed HTTPS origin or credentials in a URL. */
function originMatches(details: BrowserPasskeyDetails): boolean {
  const url = details.frame && URL.parse(details.frame.url);
  if (!url || details.frame!.parent || url.username || url.password) return false;
  return url.protocol === 'https:' && url.origin === details.origin;
}
/** Resolve a real current top frame in the active personal session before creating any native lease. */
function sourceFor(deps: BrowserPasskeyDeps, details: BrowserPasskeyDetails): PasskeySource | null {
  const contents = deps.electron.webContents.fromFrame(details.frame!);
  if (!contents || contents.mainFrame !== details.frame || !personal(deps, contents)) return null;
  return passkeySource(deps, contents, details.frame!);
}

/** Experimental account choices carry the exact native attempt, unlike ordinary stock account events. */
type NativeAccount = SelectWebauthnAccountDetails & {
  /** Bound by the native delegate, never page IPC. */ requestId?: string;
};

/** One application-owned lease survives start allow until native end; cancel never frees an OS controller by itself. */
export class BrowserPasskeys {
  /** The root services remain live across profile/control changes. */
  private readonly deps: BrowserPasskeyDeps;
  /** Sessions reused by a profile must not register duplicate native policy handlers. */
  private readonly installed = new WeakSet<Session>();
  /** Refuse a competing ceremony rather than queueing consent or native callbacks. */
  private pending?: BrowserPasskeyLease;
  /** This adapter is created only after the runtime's explicit experimental capability is verified. */
  constructor(deps: BrowserPasskeyDeps) {
    this.deps = deps;
  }
  /** Policy events are main-only and never become shell IPC or agent actions. */
  install(session: Session): void {
    if (this.installed.has(session)) return;
    this.installed.add(session);
    const native = session as unknown as BrowserPasskeyEvents;
    native.on('oya-webauthn-request', (_event, details, decide, cancel) =>
      this.receive({ session, details, decide, cancel }),
    );
    native.on('oya-webauthn-ended', (_event, details) => this.ended(session, details));
  }
  /** Decide exactly once inside native dispatch; an exception after allow cannot leave the attempt authorized. */
  private receive(call: BrowserPasskeyCall): void {
    const allowed = this.evaluate(call);
    if (!allowed) this.reject(call.session, call.details, call.cancel);
    this.decide(call, allowed);
  }
  /** Unavailable native frames/services fail closed with no raw exception in UI or diagnostics. */
  private evaluate({ session, details, cancel }: BrowserPasskeyCall): boolean {
    try {
      return typeof cancel === 'function' && this.allowed(session, details, cancel);
    } catch {
      return false;
    }
  }
  /** A broken decision callback must revoke, even when it recorded allow before throwing. */
  private decide(call: BrowserPasskeyCall, allowed: boolean): void {
    try {
      call.decide(allowed);
    } catch {
      this.reject(call.session, call.details, call.cancel);
    }
  }
  /** Native validation remains authoritative; this gate only adds Oya ownership and exact source checks. */
  private allowed(session: Session, details: BrowserPasskeyDetails, cancel: BrowserPasskeyCancel): boolean {
    if (!validDetails(details) || !originMatches(details)) return false;
    if (details.phase === 'complete') return this.complete(session, details);
    if (this.pending || !details.frame) return false;
    return this.start(session, details, cancel);
  }
  /** No native discovery may start from an unfocused owner or a session different from the original frame. */
  private start(session: Session, details: BrowserPasskeyDetails, cancel: BrowserPasskeyCancel): boolean {
    const source = sourceFor(this.deps, details);
    if (!source || source.session !== session || !source.window.isFocused()) return false;
    this.pending = new BrowserPasskeyLease({ deps: this.deps, session, source, details, cancel });
    return this.pending.current();
  }
  /** OS-preselected accounts still cross this final gate before Chromium may return a successful assertion. */
  private complete(session: Session, details: BrowserPasskeyDetails): boolean {
    const pending = this.pending;
    return (
      !!pending &&
      personal(this.deps, pending.request.source.contents) &&
      pending.matches(session, details) &&
      pending.approveCompletion()
    );
  }
  /** Policy denial revokes only that native attempt, preserving an unrelated already-running ceremony. */
  private reject(session: Session, details: BrowserPasskeyDetails, cancel: BrowserPasskeyCancel): void {
    if (this.pending?.session === session && this.pending.requestId === details?.requestId) this.pending.cancel();
    else {
      try {
        if (typeof cancel === 'function') cancel();
      } catch {
        /* Native dispatch also denies. */
      }
    }
  }
  /** A terminal event cannot dispose another session's pending native operation. */
  private ended(session: Session, details: BrowserPasskeyEnded): void {
    const pending = this.pending;
    if (!pending || pending.session !== session || pending.requestId !== details?.requestId) return;
    this.pending = undefined;
    pending.dispose();
  }
  /** Account choice belongs to the same original native attempt when the experiment gates a Touch ID assertion. */
  account(details: SelectWebauthnAccountDetails): boolean {
    const id = (details as NativeAccount).requestId;
    if (!this.pending) return id === undefined;
    return (
      id === this.pending.requestId &&
      details.frame === this.pending.request.source.frame &&
      this.pending.mayChooseAccount()
    );
  }
  /** Control/profile/recording transitions revoke before any await; a return to the same owner does not revive it. */
  sync(): void {
    const pending = this.pending;
    if (pending && (!pending.current() || !personal(this.deps, pending.request.source.contents))) pending.cancel();
  }
}
