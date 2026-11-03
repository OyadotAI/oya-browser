/** Account-correlated setup requests prevent stale responses and automatic purchase linking. */
'use client';
import { useEffect, useMemo, useRef, useState, type Dispatch, type RefObject, type SetStateAction } from 'react';
import { useAuth } from '@/components/auth-provider';
import { callbackProblem, requestSetup, type SetupViewState } from './requests';

/** Mutable request ownership is private to this page instance. */
interface Handle {
  /** The account whose response may be rendered. */
  token: string | null;
  /** Aborting suppresses stale results on account changes and unmount. */
  active: RefObject<AbortController | null>;
  /** Stable idempotency hint for explicit connection retries. */
  key: RefObject<string>;
  /** Only a completed status read chooses the purchase a click may connect. */
  context: RefObject<string>;
  /** Updates the page only while this request still owns it. */
  set: Dispatch<SetStateAction<SetupViewState>>;
}

/** The same card remains visible during background status checks. */
const initial = (token: string | null): SetupViewState => ({ kind: 'loading', forToken: token, busy: true });

/** A lost connection says nothing about whether a durable link was written. */
function networkFailure(handle: Handle, controller: AbortController) {
  if (!controller.signal.aborted) handle.set({ kind: 'offline', forToken: handle.token, busy: false });
}

/** Returns a cancellation owner; a second explicit click cannot create another in-flight mutation. */
function startRequest(handle: Handle, connect = false) {
  if (connect && handle.active.current) return;
  handle.active.current?.abort();
  const controller = new AbortController();
  handle.active.current = controller;
  handle.set((state) => ({ ...state, busy: true }));
  void completeRequest(handle, controller, connect);
  return () => controller.abort();
}

/** Settles only its own account/request generation and releases its local slot. */
async function completeRequest(handle: Handle, controller: AbortController, connect: boolean) {
  try {
    const state = await readOrConnect(handle, controller.signal, connect);
    adoptState(handle, controller.signal, state);
  } catch {
    networkFailure(handle, controller);
  } finally {
    if (handle.active.current === controller) handle.active.current = null;
  }
}

/** Reads can replace the displayed context; a mutation carries the previously displayed one. */
function readOrConnect(handle: Handle, signal: AbortSignal, connect: boolean) {
  const consent = connect ? { key: handle.key.current, contextId: handle.context.current } : undefined;
  return requestSetup(handle.token, signal, consent);
}

/** A response never changes consent after it loses ownership of the page. */
function adoptState(handle: Handle, signal: AbortSignal, state: SetupViewState) {
  if (signal.aborted) return;
  handle.context.current = state.contextId || '';
  handle.set(state);
}

/** Captures a safe callback failure before issuing any account-specific request. */
function beginLoad(handle: Handle) {
  const problem = callbackProblem();
  if (!problem) return startRequest(handle);
  queueMicrotask(() => handle.set({ kind: problem, forToken: handle.token, busy: false }));
}

/** Auth restoration and account changes cancel the previous request before starting another. */
function useInitialLoad(handle: Handle, loading: boolean) {
  useEffect(() => {
    if (!loading) return beginLoad(handle);
  }, [handle, loading]);
}

/** Focus changes only after a deliberate connection completes, never on status refresh. */
function useSetupHeading(state: SetupViewState) {
  const heading = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    if (state.connected) heading.current?.focus();
  }, [state.connected]);
  return heading;
}

/** Exposes explicit connection and read-only retry actions, plus account-safe rendering state. */
function useRequests(token: string | null, loading: boolean) {
  const [state, set] = useState<SetupViewState>(initial(null));
  const active = useRef<AbortController | null>(null),
    key = useRef(crypto.randomUUID()),
    context = useRef('');
  const handle = useMemo(() => ({ token, active, key, context, set }), [token]);
  useInitialLoad(handle, loading);
  return { state, handle };
}

/** Explicit UI actions never submit on authentication or context restoration. */
function requestActions(handle: Handle) {
  return { refresh: () => startRequest(handle), connect: () => startRequest(handle, true) };
}

/** Exposes only the current account's state and deliberate account-linking actions. */
export function useMarketplace() {
  const auth = useAuth();
  const { state, handle } = useRequests(auth.token, auth.loading);
  const visible = auth.loading || state.forToken !== auth.token ? initial(auth.token) : state;
  const heading = useSetupHeading(visible);
  return { auth, state: visible, heading, ...requestActions(handle) };
}
