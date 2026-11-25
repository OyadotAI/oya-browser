/**
 * Claiming an agent's key. The key arrives in the link's fragment and moves
 * to this tab's storage at once, so it is off the URL and survives a sign-in.
 * Nothing is sent until the signed-in person confirms: a link is something
 * anyone can write, and opening one must not change an account by itself.
 */
'use client';

import { useEffect, useRef, useState } from 'react';
import { useAuth } from '@/components/auth-provider';
import { AccountError, claimAgentKey } from '@/lib/api';
import { keepStepUp, stepUpPending } from '@/lib/auth/storage';
import { stepUpUrl } from '@/lib/auth/step-up';
import { claimHint, forgetClaim, pendingClaim, rememberClaim } from '@/lib/claim';
import { Status } from '@/lib/http-status';

/** Where the claim stands. */
export interface ClaimState {
  /** Waiting on the session, on a sign-in, on the person's answer or on the server; or how it ended. */
  status: 'loading' | 'signed-out' | 'confirm' | 'claiming' | 'claimed' | 'discarded' | 'missing' | 'failed';
  /** Why it failed, or why the last try did not go through. */
  error?: string;
  /** The start of the key being claimed, so the person can check it against their agent's. */
  hint?: string;
}

/** What the claim page shows and the two answers the person can give. */
export interface Claim {
  /** Where the claim stands. */
  state: ClaimState;
  /** Claims the waiting key for the signed-in person. */
  confirm: () => void;
  /** Forgets the waiting key without claiming it. */
  discard: () => void;
}

/** Reports the next state. */
type Done = (state: ClaimState) => void;

/** What a try that never reached the server says; the key is kept so they can try again. */
const UNREACHABLE = 'Could not reach Oya. Check your connection and try again.';

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

/**
 * The answers that mean this key will never be claimed, so it is forgotten.
 * Anything else (a lapsed session, a rate limit, a server failure, no answer)
 * may pass, and losing the key then would mean asking the agent for a new link.
 */
const FINAL: readonly number[] = [Status.BAD_REQUEST, Status.FORBIDDEN, Status.NOT_FOUND, Status.CONFLICT];

/** Whether the server answered and said no for good. */
const refused = (err: unknown) => err instanceof AccountError && FINAL.includes(err.status ?? 0);

/** Handles a failed claim: steps up when a second factor is owed, ends on a refusal, else offers another try. */
function failClaim(err: unknown, key: string, done: Done) {
  if (err instanceof AccountError && err.code === 'mfa_required') return requestStepUp();
  if (!refused(err)) return done({ status: 'confirm', hint: claimHint(key), error: UNREACHABLE });
  forgetClaim();
  done({ status: 'failed', error: (err as Error).message });
}

/** Claims `key` for the signed-in person and reports how it went. */
async function claim(token: string, key: string, done: Done) {
  done({ status: 'claiming', hint: claimHint(key) });
  try {
    await claimAgentKey(token, key);
    forgetClaim();
    done({ status: 'claimed' });
  } catch (err) {
    failClaim(err, key, done);
  }
}

/** What to show once the session is known: the question, a sign-in, or that there is no key. */
function settle(token: string | null, done: Done) {
  const key = takeKeyFromUrl();
  if (key && token && stepUpPending()) return window.location.replace(stepUpUrl('/claim'));
  if (key && token) return done({ status: 'confirm', hint: claimHint(key) });
  done({ status: key ? 'signed-out' : 'missing' });
}

/** Forgets the waiting key and says so. */
function discardClaim(done: Done) {
  forgetClaim();
  done({ status: 'discarded' });
}

/** Claims the waiting key, if there still is one and someone is signed in. */
function confirmClaim(token: string | null, done: Done) {
  const key = pendingClaim();
  if (token && key) void claim(token, key, done);
}

/** Settles the page once the session is known, and once more after a sign-in; never again after that. */
function useSettle(token: string | null, loading: boolean, done: Done) {
  const asked = useRef(false);
  useEffect(() => {
    if (loading || asked.current) return;
    asked.current = Boolean(token);
    queueMicrotask(() => settle(token, done));
  }, [loading, token, done]);
}

/** Takes the key off the URL, then asks the signed-in person before claiming it. */
export function useClaim(): Claim {
  const { token, loading } = useAuth();
  const [state, setState] = useState<ClaimState>({ status: 'loading' });
  useSettle(token, loading, setState);
  return { state, confirm: () => confirmClaim(token, setState), discard: () => discardClaim(setState) };
}
