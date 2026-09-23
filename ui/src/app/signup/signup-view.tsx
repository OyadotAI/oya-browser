/**
 * The sign-up page's body: Google or GitHub, or create an account with email,
 * then on to the console.
 */
'use client';

import Link from 'next/link';
import { AuthLoading, AuthShell } from '@/components/auth/auth-shell';
import { OAuthButtons } from '@/components/auth/oauth-buttons';
import { SignupForm } from './signup-form';
import { useSignup } from './use-signup';

/** The page's settings from the server. */
interface ViewProps {
  /** The Turnstile site key, '' when the captcha is off. */
  siteKey: string;
}

/** Create an account; `siteKey` turns the captcha on. */
export function SignupView({ siteKey }: ViewProps) {
  const { auth, form, fields, captcha, submit } = useSignup(siteKey);
  // Don't render until auth state is resolved
  if (auth.loading || auth.user) return <AuthLoading />;
  const footer = (
    <>
      Already have an account?{' '}
      <Link href="/login" className="font-medium text-accent hover:text-accent-hover">
        Sign in
      </Link>
    </>
  );
  return (
    <AuthShell glow="indigo" footer={footer}>
      <h1 className="mb-1 font-display text-2xl font-bold text-text">Create your account</h1>
      <p className="mb-8 text-sm text-text-muted">Get started with Oya Browser in seconds</p>
      <OAuthButtons />
      <SignupForm fields={fields} form={form} captcha={captcha} onSubmit={submit} />
    </AuthShell>
  );
}
