/**
 * Claiming an agent's key. The key arrives in the link's fragment and moves
 * to this tab's storage at once, so it is off the URL and survives a sign-in;
 * once someone is signed in it is imported to their account, which is what
 * lifts the agent key's limits (cloud browsers need a person).
 */
'use client';

import { useEffect, useState } from 'react';
import { useAuth } from '@/components/auth-provider';
import { importApiKey } from '@/lib/api';
import { forgetClaim, pendingClaim, rememberClaim } from '@/lib/claim';

/** Where the claim stands. */
export interface ClaimState {
  /** Waiting on the session or the import, done, refused, or waiting on a sign-in. */
  status: 'signed-out' | 'claiming' | 'claimed' | 'missing' | 'failed';
  /** Why it failed, when it did. */
  error?: string;
}

/** Moves a key in the fragment to storage and off the URL; the key waiting to be claimed, or ''. */
function takeKeyFromUrl() {
  const fromLink = window.location.hash.slice(1);
  if (fromLink) rememberClaim(fromLink);
  if (fromLink) window.history.replaceState(null, '', window.location.pathname);
  return pendingClaim();
}

/** Imports `key` for the signed-in person and reports how it went. */
async function claim(token: string, key: string, done: (state: ClaimState) => void) {
  try {
    await importApiKey(token, key, 'Agent');
    forgetClaim();
    done({ status: 'claimed' });
  } catch (err) {
    forgetClaim();
    done({ status: 'failed', error: err instanceof Error ? err.message : 'Could not claim this key' });
  }
}

/** Claims the waiting key if there is one and someone is signed in, else says which of the two is missing. */
function settle(token: string | null, done: (state: ClaimState) => void) {
  const key = takeKeyFromUrl();
  if (key && token) return void claim(token, key, done);
  const status = key ? 'signed-out' : 'missing';
  queueMicrotask(() => done({ status }));
}

/** Takes the key off the URL, then claims it once the session is known; what to show meanwhile. */
export function useClaim(): ClaimState {
  const { token, loading } = useAuth();
  const [state, setState] = useState<ClaimState>({ status: 'claiming' });
  useEffect(() => {
    if (!loading) settle(token, setState);
  }, [loading, token]);
  return state;
}
