/**
 * The sign-in page's body: Google or GitHub, an account (email and password)
 * or, for a self-hosted deployment without accounts, an API key.
 */
'use client';

import Link from 'next/link';
import { AuthLoading, AuthShell } from '@/components/auth/auth-shell';
import { OAuthButtons } from '@/components/auth/oauth-buttons';
import { AccountForm, KeyForm, ModeSwitch } from './login-forms';
import { AgentSetup } from './agent-setup';
import { useLogin, type LoginMode } from './use-login';

/** The line under the heading, per mode. */
const SUBTITLES: Record<LoginMode, string> = {
  account: 'Sign in to your account to continue',
  key: 'Paste an API key from this deployment',
  agent: 'Give your AI agent a browser',
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
      Don&apos;t have an account?{' '}
      <Link href="/signup" className="font-medium text-accent hover:text-accent-hover">
        Sign up
      </Link>
    </>
  );
  return (
    <AuthShell glow="accent" footer={footer}>
      <h1 className="mb-1 font-display text-2xl font-bold text-text">Welcome back</h1>
      <p className="mb-6 text-sm text-text-muted">{SUBTITLES[fields.mode]}</p>
      <ModeSwitch fields={fields} form={form} />
      {fields.mode === 'agent' && <AgentSetup />}
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
