/**
 * Claiming an agent's key. The key arrives in the link's fragment and moves
 * to this tab's storage at once, so it is off the URL and survives a sign-in;
 * once someone is signed in it is imported to their account, which is what
 * lifts the agent key's limits (cloud browsers need a person).
 */
'use client';

import { useEffect, useRef, useState, type RefObject } from 'react';
import { useAuth } from '@/components/auth-provider';
import { AccountError, importApiKey } from '@/lib/api';
import { keepStepUp, stepUpPending } from '@/lib/auth/storage';
import { stepUpUrl } from '@/lib/auth/step-up';
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

/** Redirects to the 2FA page while keeping the pending claim in storage. */
function requestStepUp() {
  keepStepUp(true);
  window.location.replace(stepUpUrl('/claim'));
}

/** Handles a refused import, preserving the claim when a second factor is owed. */
function failClaim(err: unknown, done: (state: ClaimState) => void) {
  if (err instanceof AccountError && err.code === 'mfa_required') return requestStepUp();
  forgetClaim();
  done({ status: 'failed', error: err instanceof Error ? err.message : 'Could not claim this key' });
}

/** Imports `key` for the signed-in person and reports how it went. */
async function claim(token: string, key: string, done: (state: ClaimState) => void) {
  try {
    await importApiKey(token, key, 'Agent');
    forgetClaim();
    done({ status: 'claimed' });
  } catch (err) {
    failClaim(err, done);
  }
}

/** Marks the claim as started and imports `key` for the signed-in person. */
function beginClaim(token: string, key: string, started: RefObject<boolean>, done: (state: ClaimState) => void) {
  started.current = true;
  void claim(token, key, done);
}

/** Claims the waiting key if there is one and someone is signed in, else says which of the two is missing. */
function settle(token: string | null, started: RefObject<boolean>, done: (state: ClaimState) => void) {
  const key = takeKeyFromUrl();
  if (key && token && stepUpPending()) return window.location.replace(stepUpUrl('/claim'));
  if (key && token) return beginClaim(token, key, started, done);
  const status = key ? 'signed-out' : 'missing';
  queueMicrotask(() => done({ status }));
}

/** Takes the key off the URL, then claims it once the session is known; what to show meanwhile. */
export function useClaim(): ClaimState {
  const { token, loading } = useAuth();
  const started = useRef(false);
  const [state, setState] = useState<ClaimState>({ status: 'claiming' });
  useEffect(() => {
    if (!loading && !started.current) settle(token, started, setState);
  }, [loading, token]);
  return state;
}
