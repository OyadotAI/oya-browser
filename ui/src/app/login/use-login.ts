/**
 * The sign-in page's state and submits. A self-hosted deployment has API keys
 * and no accounts, so a key is a first-class way in, it is the identity the
 * whole dashboard is scoped to.
 */
'use client';

import { useState, type FormEvent } from 'react';
import { apiUrl, authHeaders, CONSOLE_KEY } from '@/lib/api';
import { Status } from '@/lib/http-status';
import {
  firstProblem,
  HOME,
  runSubmit,
  useFormState,
  useSignedInRedirect,
  type Submission,
} from '@/components/auth/use-auth-form';

/** The two ways in. */
export type LoginMode = 'account' | 'key';

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

/** Signing in with an account: both fields are needed. */
function accountSubmission(fields: Fields, auth: Auth): Submission {
  return {
    problem: firstProblem([
      [!fields.email.trim(), 'Email is required'],
      [!fields.password, 'Password is required'],
    ]),
    action: () => auth.login(fields.email, fields.password).then(() => auth.router.replace(HOME)),
    fallback: 'Invalid email or password',
  };
}

/** Everything the sign-in page renders from. */
export function useLogin() {
  const auth = useSignedInRedirect();
  const form = useFormState();
  const fields = useLoginFields();
  const submitKey = (e: FormEvent) => runSubmit(e, form, keySubmission(fields, auth));
  const submitAccount = (e: FormEvent) => runSubmit(e, form, accountSubmission(fields, auth));
  return { auth, form, fields, submitKey, submitAccount };
}
