/**
 * The sign-in page's state and submits. A self-hosted deployment has API keys
 * and no accounts, so a key is a first-class way in, it is the identity the
 * whole dashboard is scoped to.
 */
'use client';

import { useState, type FormEvent } from 'react';
import { event } from '@/lib/analytics';
import { apiUrl, authHeaders, CONSOLE_KEY } from '@/lib/api';
import { Status } from '@/lib/http-status';
import { useTurnstile, type Captcha } from '@/components/auth/use-turnstile';
import {
  firstProblem,
  afterSignIn,
  HOME,
  runSubmit,
  useFormState,
  useSignedInRedirect,
  type Submission,
} from '@/components/auth/use-auth-form';

/** The three ways in: a person's account, a machine's API key, or an agent's setup. */
export type LoginMode = 'account' | 'key' | 'agent';

/**
 * Prove the server accepts the key before storing it, so a wrong key fails
 * here rather than as an empty dashboard.
 */
async function verifyKey(key: string) {
  const res = await fetch(apiUrl('/config'), { headers: authHeaders(key) });
  if (!res.ok) {
    throw new Error(
      res.status === Status.UNAUTHORIZED ? 'That key was rejected' : `Could not verify the key (${res.status})`,
    );
  }
  // sessionStorage: a fleet administrator credential should not outlive the tab.
  sessionStorage.setItem(CONSOLE_KEY, key);
}

/** The typed values. */
function useLoginFields() {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [apiKey, setApiKey] = useState('');
  const [mode, setMode] = useState<LoginMode>('account');
  return { email, setEmail, password, setPassword, apiKey, setApiKey, mode, setMode };
}

/** The auth state plus the router, as useSignedInRedirect gives them. */
type Auth = ReturnType<typeof useSignedInRedirect>;
/** The typed values and their setters. */
type Fields = ReturnType<typeof useLoginFields>;

/** Signing in with a key: it must be there, and the server must accept it. */
function keySubmission(fields: Fields, auth: Auth): Submission {
  const key = fields.apiKey.trim();
  return {
    problem: key ? '' : 'Enter an API key',
    action: () => verifyKey(key).then(() => auth.router.replace(HOME)),
    fallback: 'Could not verify the key',
  };
}

/** Signs in, counting the outcome either way, then goes home; a failure spends the captcha, so it resets. */
function signIn(auth: Auth, fields: Fields, captcha: Captcha) {
  return auth.login(fields.email, fields.password, captcha.token || undefined).then(
    () => (event('sign_in_success'), auth.router.replace(afterSignIn())),
    (err) => (event('sign_in_failed'), captcha.reset(), Promise.reject(err)),
  );
}

/** Both fields are needed, and the captcha when it is on. */
function accountProblem(fields: Fields, captcha: Captcha) {
  return firstProblem([
    [!fields.email.trim(), 'Email is required'],
    [!fields.password, 'Password is required'],
    [captcha.missing, 'Please complete the captcha check'],
  ]);
}

/** Signing in with an account: both fields are needed. */
function accountSubmission(fields: Fields, auth: Auth, captcha: Captcha): Submission {
  return {
    problem: accountProblem(fields, captcha),
    action: () => signIn(auth, fields, captcha),
    fallback: 'Invalid email or password',
  };
}

/** Everything the sign-in page renders from. */
export function useLogin(siteKey = '') {
  const auth = useSignedInRedirect();
  const form = useFormState();
  const fields = useLoginFields();
  const captcha = useTurnstile(siteKey);
  const submitKey = (e: FormEvent) => runSubmit(e, form, keySubmission(fields, auth));
  const submitAccount = (e: FormEvent) => runSubmit(e, form, accountSubmission(fields, auth, captcha));
  return { auth, form, fields, captcha, submitKey, submitAccount };
}
