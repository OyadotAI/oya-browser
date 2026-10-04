/**
 * Two-factor authentication: set up an authenticator app, enter its code to
 * finish signing in (or to reach a page that asks for it, such as the admin
 * page), and see or remove the apps on the account. Its state lives in
 * `use-mfa.ts`.
 */
'use client';

import Link from 'next/link';
import { AuthLoading, AuthShell } from '@/components/auth/auth-shell';
import { CodeForm, ManageView, SetupView } from './mfa-views';
import { useMfa, type MfaMode, type MfaState } from './use-mfa';

/** One view: its heading and what is under it. */
interface View {
  /** The heading. */
  title: string;
  /** The body, drawn from the page's state. */
  body: (s: MfaState) => React.ReactNode;
}

/** Each view, by mode. */
const VIEWS: Record<Exclude<MfaMode, 'loading'>, View> = {
  code: { title: 'Enter your code', body: (s) => <CodeForm s={s} /> },
  setup: { title: 'Set up two-factor authentication', body: (s) => <SetupView s={s} /> },
  manage: { title: 'Two-factor authentication', body: (s) => <ManageView s={s} /> },
};

/** The two-factor page. */
export default function MfaPage() {
  const s = useMfa();
  const footer = <Link href="/dashboard">Back to the console</Link>;
  if (s.signedOut)
    return (
      <AuthShell glow="accent" footer={footer}>
        <Link className="text-accent" href="/login">
          Sign in
        </Link>{' '}
        to set up two-factor authentication.
      </AuthShell>
    );
  if (s.mode === 'loading')
    return s.error ? (
      <AuthShell glow="accent" footer={footer}>
        {s.error}
      </AuthShell>
    ) : (
      <AuthLoading />
    );
  const view = VIEWS[s.mode];
  return (
    <AuthShell glow="accent" footer={footer}>
      <h1 className="mb-6 text-xl font-semibold text-text">{view.title}</h1>
      {view.body(s)}
    </AuthShell>
  );
}
