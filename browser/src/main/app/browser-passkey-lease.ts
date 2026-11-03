/** Keep a native cancellation closure alive throughout one modal ceremony without retaining credential data. */
import type { Session } from 'electron';
import { PasskeyRequest, type PasskeySource } from './passkey-request.ts';
import type { BrowserPasskeyCancel, BrowserPasskeyDeps, BrowserPasskeyDetails } from './browser-passkey-types.ts';

/** A construction scope keeps native provenance separate from the request metadata. */
interface LeaseInput {
  /** Live root ownership services. */ deps: BrowserPasskeyDeps;
  /** Original session. */ session: Session;
  /** Original native source. */ source: PasskeySource;
  /** Validated native attempt. */ details: BrowserPasskeyDetails;
  /** Revocation only, never assertion material. */ cancel: BrowserPasskeyCancel;
}

/** Native request ownership is separate from OS consent and the later account chooser. */
export class BrowserPasskeyLease {
  /** Exact session which emitted the validated request. */
  readonly session: Session;
  /** Exact native attempt; a same-frame replacement cannot borrow its consent. */
  readonly requestId: string;
  /** Original browser-owned frame/window/profile/opener provenance. */
  readonly request: PasskeyRequest;
  /** Literal native RP/origin remain fixed through both policy phases. */
  private readonly relyingPartyId: string;
  /** A redirected/replaced document cannot relabel the original assertion. */
  private readonly origin: string;
  /** Retain only native cancellation authority, never an assertion result callback. */
  private nativeCancel?: BrowserPasskeyCancel;
  /** Terminal retirement is permanent even if profile/control returns later. */
  private disposed = false;
  /** Final policy approval is one-use even before the native terminal notification arrives. */
  private completed = false;

  /** Install existing bounded navigation/owner/timeout guards before authorizing native discovery. */
  constructor({ deps, session, source, details, cancel }: LeaseInput) {
    this.session = session;
    this.requestId = details.requestId;
    this.relyingPartyId = details.relyingPartyId;
    this.origin = details.origin;
    this.nativeCancel = cancel;
    this.request = scopedRequest(deps, source, this.abort);
  }
  /** Completion must refer to the identical request/frame/session and still-current native owner. */
  matches(session: Session, details: BrowserPasskeyDetails): boolean {
    return (
      session === this.session &&
      details.requestId === this.requestId &&
      details.frame === this.request.source.frame &&
      details.relyingPartyId === this.relyingPartyId &&
      details.origin === this.origin &&
      this.current()
    );
  }
  /** OS-owned sheets may temporarily unfocus their parent; exact source and opener ownership still apply. */
  current(): boolean {
    if (this.disposed || !this.request.current()) return false;
    if (this.request.source.contents.mainFrame !== this.request.source.frame) return false;
    const url = URL.parse(this.request.source.frame.url);
    return url?.origin === this.origin;
  }
  /** Native success is authorized once; the actual assertion remains owned by Chromium. */
  approveCompletion(): boolean {
    if (this.completed || !this.current()) return false;
    this.completed = true;
    return true;
  }
  /** An account event cannot start another chooser after final success was already approved. */
  mayChooseAccount(): boolean {
    return !this.completed && this.current();
  }
  /** Reentrant native end/error cannot cause a second cancellation or target a later request. */
  private readonly abort = (): void => {
    const cancel = this.nativeCancel;
    this.nativeCancel = undefined;
    try {
      cancel?.();
    } catch {
      /* Final native policy still refuses the aborted source. */
    } finally {
      this.request.dispose();
    }
  };
  /** Main's control/profile/recording callbacks call this synchronously before awaiting cleanup. */
  cancel(): void {
    this.request.cancel();
  }
  /** Native end removes listeners and timer without calling an obsolete native cancellation closure. */
  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.nativeCancel = undefined;
    this.request.controller.signal.removeEventListener('abort', this.abort);
    this.request.dispose();
  }
}

/** Attach persistent cancellation before native admission may return true. */
function scopedRequest(deps: BrowserPasskeyDeps, source: PasskeySource, cancel: () => void): PasskeyRequest {
  const request = new PasskeyRequest(deps, source);
  request.controller.signal.addEventListener('abort', cancel);
  return request;
}
