/**
 * Where Google and GitHub sign-ins land: a spinner while the session starts,
 * or what went wrong with a way back to the sign-in page.
 */
'use client';

import Link from 'next/link';
import { AuthLoading, AuthShell } from '@/components/auth/auth-shell';
import { useOAuthCallback } from './use-oauth-callback';

/** Finishes the sign-in, then on to the console. */
export default function OAuthCallbackPage() {
  const error = useOAuthCallback();
  if (!error) return <AuthLoading />;
  const footer = (
    <Link href="/login" className="font-medium text-accent hover:text-accent-hover">
      Back to sign in
    </Link>
  );
  return (
    <AuthShell glow="accent" footer={footer}>
      <h1 className="mb-2 font-display text-2xl font-bold text-text">Sign-in failed</h1>
      <p role="alert" className="text-sm text-text-muted">
        {error}
      </p>
    </AuthShell>
  );
}
