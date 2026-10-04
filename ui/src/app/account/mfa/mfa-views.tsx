/**
 * The two-factor page's views: the code form, setting up an authenticator app
 * (QR code and secret), and the list of apps with a way to remove each. They
 * draw from useMfa's state and hold none of their own.
 */
'use client';

import type { FormEvent } from 'react';
import { FormError, SubmitButton } from '@/components/auth/fields';
import { TOTP_CODE_LENGTH } from '@/lib/constants';
import type { MfaState } from './use-mfa';

/** Every view's props. */
interface ViewProps {
  /** The page's state. */
  s: MfaState;
}

/** Submits the code without reloading the page. */
const onCode = (s: MfaState) => (e: FormEvent) => (e.preventDefault(), void s.verify());

/** The six-digit code from the app, and its submit. */
export function CodeForm({ s }: ViewProps) {
  return (
    <form onSubmit={onCode(s)} className="flex flex-col gap-3">
      <label className="flex flex-col gap-1.5">
        <span className="text-sm font-medium text-text-muted">6-digit code</span>
        <input
          value={s.code}
          onChange={(e) => s.setCode(e.target.value)}
          inputMode="numeric"
          autoComplete="one-time-code"
          maxLength={TOTP_CODE_LENGTH}
          pattern="[0-9]*"
          autoFocus
          className="h-10 rounded-lg border border-border bg-bg px-3 text-center font-mono text-lg tracking-[0.4em] text-text outline-none focus:border-accent"
        />
      </label>
      <FormError error={s.error} />
      <SubmitButton busy={s.busy} label="Verify" busyLabel="Checking" />
    </form>
  );
}

/** A button styled as a secondary action. */
const SECONDARY =
  'rounded-lg border border-border px-4 py-2 text-sm text-text transition-colors hover:bg-bg-elevated disabled:opacity-60';

/** Adding an app: first the offer, then the QR code, its secret and the first code. */
export function SetupView({ s }: ViewProps) {
  if (!s.enrolment)
    return (
      <div className="flex flex-col gap-3">
        <p className="text-sm text-text-muted">
          Use an authenticator app (1Password, Google Authenticator, Authy) for a code at every sign-in.
        </p>
        <FormError error={s.error} />
        <button onClick={() => void s.start()} disabled={s.busy} className={SECONDARY}>
          Set up an authenticator app
        </button>
      </div>
    );
  return (
    <div className="flex flex-col gap-4">
      <p className="text-sm text-text-muted">Scan this with your app, then enter the code it shows.</p>
      {/* eslint-disable-next-line @next/next/no-img-element -- an inline SVG data URI, nothing to optimize */}
      <img
        src={s.enrolment.qr_code}
        alt="QR code for your authenticator app"
        className="mx-auto h-44 w-44 bg-white p-2"
      />
      <p className="text-xs text-text-dim">
        Cannot scan? Enter this key: <code className="break-all font-mono text-text">{s.enrolment.secret}</code>
      </p>
      <CodeForm s={s} />
    </div>
  );
}

/** The person's apps, each removable, and a way to add another. */
export function ManageView({ s }: ViewProps) {
  return (
    <div className="flex flex-col gap-3">
      <p className="text-sm text-emerald-400">Two-factor authentication is on.</p>
      <ul className="flex flex-col gap-2">
        {s.status?.factors
          .filter((f) => f.status === 'verified')
          .map((f) => (
            <li key={f.id} className="flex items-center justify-between gap-3 rounded-md border border-border p-3">
              <span className="truncate text-sm text-text">{f.name || 'Authenticator app'}</span>
              <button onClick={() => void s.remove(f.id)} disabled={s.busy} className="text-sm text-red-400">
                Remove
              </button>
            </li>
          ))}
      </ul>
      <FormError error={s.error} />
      <button onClick={() => void s.start()} disabled={s.busy} className={SECONDARY}>
        Add another app
      </button>
    </div>
  );
}
