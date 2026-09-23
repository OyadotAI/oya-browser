/**
 * The sign-up page's state and submit.
 */
'use client';

import { useState, type FormEvent } from 'react';
import { event } from '@/lib/analytics';
import {
  firstProblem,
  afterSignIn,
  runSubmit,
  useFormState,
  useSignedInRedirect,
  type Submission,
} from '@/components/auth/use-auth-form';
import { useTurnstile, type Captcha } from '@/components/auth/use-turnstile';

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

/** Email, a long enough password and the captcha (when on) are needed; the name is optional. */
function signupProblem(f: SignupFields, captcha: Captcha): string {
  return firstProblem([
    [!f.email.trim(), 'Email is required'],
    [!f.password, 'Password is required'],
    [f.password.length < MIN_PASSWORD_LENGTH, `Password must be at least ${MIN_PASSWORD_LENGTH} characters`],
    [captcha.missing, 'Please complete the captcha check'],
  ]);
}

/** Creates the account and goes to the console; a failure spends the captcha, so it resets. */
function createAccount(f: SignupFields, name: string | undefined, auth: Auth, captcha: Captcha) {
  return auth.signup(f.email, f.password, name, captcha.token || undefined).then(
    () => (event('sign_up_success'), auth.router.replace(afterSignIn())),
    (err) => (captcha.reset(), Promise.reject(err)),
  );
}

/** The auth state plus the router. */
type Auth = ReturnType<typeof useSignedInRedirect>;

/** Creating the account, then going to the console. */
function signupSubmission(f: SignupFields, auth: Auth, captcha: Captcha): Submission {
  const name = f.displayName.trim() || undefined;
  return {
    problem: signupProblem(f, captcha),
    action: () => createAccount(f, name, auth, captcha),
    fallback: 'Something went wrong. Please try again.',
  };
}

/** Everything the sign-up page renders from. */
export function useSignup(siteKey = '') {
  const auth = useSignedInRedirect();
  const form = useFormState();
  const fields = useSignupFields();
  const captcha = useTurnstile(siteKey);
  const submit = (e: FormEvent) => runSubmit(e, form, signupSubmission(fields, auth, captcha));
  return { auth, form, fields, captcha, submit };
}
