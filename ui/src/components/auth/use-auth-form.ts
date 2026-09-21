/**
 * What the sign-in and sign-up forms share: error and busy state, a submit
 * runner, and the redirect away once someone is signed in.
 */
'use client';

import { useEffect, useState, type FormEvent } from 'react';
import { useRouter } from 'next/navigation';
import { useAuth } from '@/components/auth-provider';

/** Where a signed-in person goes. */
export const HOME = '/dashboard';

/** A form's error and busy state. */
export interface FormState {
  /** The error line, empty for none. */
  error: string;
  /** Sets the error line. */
  setError: (error: string) => void;
  /** Whether a submit is in flight. */
  submitting: boolean;
  /** Sets whether a submit is in flight. */
  setSubmitting: (busy: boolean) => void;
}

/** Error and busy state for one form. */
export function useFormState(): FormState {
  const [error, setError] = useState('');
  const [submitting, setSubmitting] = useState(false);
  return { error, setError, submitting, setSubmitting };
}

/** A setter that also clears a showing error, so typing dismisses it. */
export function clearing(form: FormState, set: (value: string) => void) {
  return (value: string) => {
    set(value);
    if (form.error) form.setError('');
  };
}

/** What one submit does. */
export interface Submission {
  /** The first validation problem, or empty to go ahead. */
  problem: string;
  /** The work, e.g. signing in and redirecting. */
  action: () => Promise<void>;
  /** The error shown when the work throws something that is not an Error. */
  fallback: string;
}

/** Handles a submit: clears the error, stops at a validation problem, else runs the action. */
export async function runSubmit(e: FormEvent, form: FormState, { problem, action, fallback }: Submission) {
  e.preventDefault();
  form.setError(problem);
  if (problem) return;
  await busyWhile(form, action, fallback);
}

/** Runs `action` with the form busy, showing its error (or `fallback`). */
async function busyWhile(form: FormState, action: () => Promise<void>, fallback: string) {
  form.setSubmitting(true);
  try {
    await action();
  } catch (err: unknown) {
    form.setError(err instanceof Error ? err.message : fallback);
  } finally {
    form.setSubmitting(false);
  }
}

/** The first failed check's message, or empty when all pass. */
export function firstProblem(checks: Array<[boolean, string]>): string {
  return checks.find(([failed]) => failed)?.[1] ?? '';
}

/** The auth state, redirecting to the console once someone is signed in. */
export function useSignedInRedirect() {
  const router = useRouter();
  const auth = useAuth();
  const { loading, user } = auth;
  useEffect(() => {
    if (!loading && user) router.replace(HOME);
  }, [loading, user, router]);
  return { ...auth, router };
}
