/** Private experimental native WebAuthn policy contracts contain no credential, challenge or assertion payload. */
import type { Session, WebFrameMain } from 'electron';
import type { AppServices } from './services.ts';
import type { PasskeyDeps } from './passkey-request.ts';

/** Browser-owned sessions and lifecycle guards are available before any experimental native discovery. */
export type BrowserPasskeyDeps = PasskeyDeps & Pick<AppServices, 'config' | 'recorder'>;

/** Only the patched runtime emits this metadata after native relying-party validation. */
export interface BrowserPasskeyDetails {
  /** Opaque identity of this native request attempt, not a credential identifier. */ requestId: string;
  /** Both policy gates are required, including an OS-preselected single account's final success. */ phase:
    'start' | 'complete';
  /** Actual requesting native frame; a detached frame cannot be replaced by a URL lookup. */ frame: WebFrameMain | null;
  /** Native-validated RP ID, never a renderer override. */ relyingPartyId: string;
  /** Actual native committed requesting origin, preserving its subdomain and port. */ origin: string;
  /** This isolated experiment implements existing-credential assertions only. */ kind: 'get';
  /** Conditional, payment and immediate presentation are not part of this modal experiment. */ presentation: 'modal';
  /** RP-intersected native transports; values do not authorize a different credential ID. */ transports: string[];
}

/** No page-visible result or success assertion is delivered by this terminal lifecycle notification. */
export interface BrowserPasskeyEnded {
  /** Only the original attempt can release its main-process ownership lease. */ requestId: string;
  /** Fixed diagnostic category; no native raw error or assertion data. */ outcome: string;
}

/** The patched native dispatch window closes before OS work or page completion can proceed. */
export type BrowserPasskeyDecision = (allowed: boolean) => void;
/** This native closure revokes the original attempt even after its start decision has been consumed. */
export type BrowserPasskeyCancel = () => void;
/** One metadata-only native dispatch; it is never retained as a renderer request or credential result. */
export interface BrowserPasskeyCall {
  /** Exact native session which owns the request. */ session: Session;
  /** Native policy metadata. */ details: BrowserPasskeyDetails;
  /** Decision closes with native dispatch. */ decide: BrowserPasskeyDecision;
  /** Native lifetime revocation survives the initial decision. */ cancel: BrowserPasskeyCancel;
}
/** Callback types remain main-process-only and are never added to the shell or agent bridge. */
export type BrowserPasskeyListener = (
  event: unknown,
  details: BrowserPasskeyDetails,
  decide: BrowserPasskeyDecision,
  cancel: BrowserPasskeyCancel,
) => void;

/** Typed extension of an actually capability-pinned native session, not a replacement implementation on stock Electron. */
export interface BrowserPasskeyEvents {
  /** Install the metadata-only policy gate. */
  on(event: 'oya-webauthn-request', listener: BrowserPasskeyListener): Session;
  /** Retire original ownership only after the native request has ended. */
  on(event: 'oya-webauthn-ended', listener: (event: unknown, details: BrowserPasskeyEnded) => void): Session;
}
