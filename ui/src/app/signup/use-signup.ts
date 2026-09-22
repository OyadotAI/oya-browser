/**
 * The sign-up page's state and submit.
 */
'use client';

import { useState, type FormEvent } from 'react';
import { event } from '@/lib/analytics';
import {
  firstProblem,
  HOME,
  runSubmit,
  useFormState,
  useSignedInRedirect,
  type Submission,
} from '@/components/auth/use-auth-form';

/** The shortest password the server accepts. */
export const MIN_PASSWORD_LENGTH = 8;

/** The typed values. */
function useSignupFields() {
  const [displayName, setDisplayName] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  return { displayName, setDisplayName, email, setEmail, password, setPassword };
}

/** The typed values and their setters. */
export type SignupFields = ReturnType<typeof useSignupFields>;

/** Email and a long enough password are needed; the name is optional. */
function signupProblem(f: SignupFields): string {
  return firstProblem([
    [!f.email.trim(), 'Email is required'],
    [!f.password, 'Password is required'],
    [f.password.length < MIN_PASSWORD_LENGTH, `Password must be at least ${MIN_PASSWORD_LENGTH} characters`],
  ]);
}

/** Creating the account, then going to the console. */
function signupSubmission(f: SignupFields, auth: ReturnType<typeof useSignedInRedirect>): Submission {
  const name = f.displayName.trim() || undefined;
  return {
    problem: signupProblem(f),
    action: () =>
      auth.signup(f.email, f.password, name).then(() => (event('sign_up_success'), auth.router.replace(HOME))),
    fallback: 'Something went wrong. Please try again.',
  };
}

/** Everything the sign-up page renders from. */
export function useSignup() {
  const auth = useSignedInRedirect();
  const form = useFormState();
  const fields = useSignupFields();
  const submit = (e: FormEvent) => runSubmit(e, form, signupSubmission(fields, auth));
  return { auth, form, fields, submit };
}
