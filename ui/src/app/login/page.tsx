/**
 * The sign-in page: an account (email and password) or, for a self-hosted
 * deployment without accounts, an API key.
 */
'use client';

import Link from 'next/link';
import { AuthLoading, AuthShell } from '@/components/auth/auth-shell';
import { AccountForm, KeyForm, ModeSwitch } from './login-forms';
import { useLogin } from './use-login';

/** Sign in, then on to the console. */
export default function LoginPage() {
  const { auth, form, fields, submitKey, submitAccount } = useLogin();
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
      <p className="mb-6 text-sm text-text-muted">
        {fields.mode === 'account' ? 'Sign in to your account to continue' : 'Paste an API key from this deployment'}
      </p>
      <ModeSwitch fields={fields} form={form} />
      {fields.mode === 'key' ? (
        <KeyForm fields={fields} form={form} onSubmit={submitKey} />
      ) : (
        <AccountForm fields={fields} form={form} onSubmit={submitAccount} />
      )}
    </AuthShell>
  );
}
