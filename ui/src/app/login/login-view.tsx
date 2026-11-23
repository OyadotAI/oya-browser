/**
 * The sign-in page's body: credentials (Google, GitHub, or email and password)
 * or, for a self-hosted deployment without accounts, an API key. Agents need no
 * tab here: they set up through the installer. Teams reach sales from sign-up's
 * Enterprise tab.
 */
'use client';

import Link from 'next/link';
import { AuthLoading, AuthShell } from '@/components/auth/auth-shell';
import { MarketplaceContext } from '@/components/auth/marketplace-context';
import { OAuthButtons } from '@/components/auth/oauth-buttons';
import { DEVELOPER_PITCH, Pitch } from '@/components/auth/pitch';
import { AccountForm, KeyForm, ModeSwitch } from './login-forms';
import { useLogin, type LoginMode } from './use-login';

/** The line under the heading, per mode. */
const SUBTITLES: Record<LoginMode, string> = {
  account: 'Sign in to your account to continue',
  key: 'Paste an API key from this deployment',
};

/** The page's settings from the server. */
interface ViewProps {
  /** The Turnstile site key, '' when the captcha is off. */
  siteKey: string;
}

/** Sign in, then on to the console; `siteKey` turns the captcha on. */
export function LoginView({ siteKey }: ViewProps) {
  const { auth, form, fields, captcha, submitKey, submitAccount } = useLogin(siteKey);
  // Don't render until auth state is resolved
  if (auth.loading || auth.user) return <AuthLoading />;
  const footer = (
    <>
      New to Oya?{' '}
      <Link href="/signup" className="font-medium text-accent hover:text-accent-hover">
        Create a free account
      </Link>
    </>
  );
  return (
    <AuthShell glow="accent" footer={footer} pitch={<Pitch content={DEVELOPER_PITCH} />}>
      <ModeSwitch fields={fields} form={form} />
      <h1 className="mb-1 font-display text-2xl font-bold text-text">Welcome back</h1>
      <p className="mb-6 text-sm text-text-muted">{SUBTITLES[fields.mode]}</p>
      <MarketplaceContext />
      {fields.mode === 'key' && <KeyForm fields={fields} form={form} onSubmit={submitKey} />}
      {fields.mode === 'account' && (
        <>
          <OAuthButtons />
          <AccountForm fields={fields} form={form} captcha={captcha} onSubmit={submitAccount} />
        </>
      )}
    </AuthShell>
  );
}
