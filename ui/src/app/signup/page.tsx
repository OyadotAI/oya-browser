/**
 * The sign-up page: create an account, then on to the console.
 */
'use client';

import Link from 'next/link';
import { AuthLoading, AuthShell } from '@/components/auth/auth-shell';
import { SignupForm } from './signup-form';
import { useSignup } from './use-signup';

/** Create an account. */
export default function SignupPage() {
  const { auth, form, fields, submit } = useSignup();
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
      <SignupForm fields={fields} form={form} onSubmit={submit} />
    </AuthShell>
  );
}
