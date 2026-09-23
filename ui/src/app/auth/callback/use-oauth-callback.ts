/**
 * Finishing a Google or GitHub sign-in. Supabase's implicit flow returns the
 * session in this page's URL fragment; its refresh token goes to the server's
 * /auth/refresh, which starts this console's own session (the httpOnly cookie)
 * exactly as an email sign-in does.
 */
'use client';

import { useEffect, useRef, useState } from 'react';
import { refreshToken } from '@/lib/api';
import { keepRefreshToken } from '@/lib/auth/storage';
import { afterSignIn } from '@/components/auth/use-auth-form';

/** The refresh token or the provider's error, from the fragment or the query. */
function readReturn() {
  const hash = new URLSearchParams(window.location.hash.slice(1));
  const query = new URLSearchParams(window.location.search);
  const pick = (key: string) => hash.get(key) || query.get(key) || '';
  return { token: hash.get('refresh_token') || '', error: pick('error_description') || pick('error') };
}

/** Starts the session from `token`, then goes to the console, whose restore picks it up. */
async function adopt(token: string) {
  const data = await refreshToken(token);
  if (data.refresh_token) keepRefreshToken(data.refresh_token);
  window.location.replace(afterSignIn());
}

/** Takes the tokens off the URL, so they are not left in history, and signs in with them. */
async function finish() {
  const { token, error } = readReturn();
  window.history.replaceState(null, '', window.location.pathname);
  if (!token) throw new Error(error || 'Sign-in did not complete');
  await adopt(token);
}

/** Runs once (twice-mounted in development); the error to show, if the sign-in fails. */
export function useOAuthCallback() {
  const [error, setError] = useState('');
  const ran = useRef(false);
  useEffect(() => {
    if (ran.current) return;
    ran.current = true;
    finish().catch((err: Error) => setError(err.message));
  }, []);
  return error;
}
