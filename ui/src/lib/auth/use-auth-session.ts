/**
 * The auth session as React hooks: state, the renewal schedule, the restore
 * on load, and the account actions. AuthProvider exposes the result.
 */
'use client';

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { logout as apiLogout, signup as apiSignup } from '../api';
import { clearSession, refreshSession, scheduleRefresh, signIn, startRestore } from './session';
import type { AuthContextType, SessionHandle, User } from './types';

/** Session state plus a stable handle over its setters and refs. */
function useSessionHandle() {
  const [user, setUser] = useState<User | null>(null);
  const [token, setToken] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const version = useRef(0);
  const handle = useMemo<SessionHandle>(() => ({ setUser, setToken, setLoading, timer, version }), []);
  return { user, token, loading, handle };
}

/** Stable clear and refresh callbacks for this session. */
function useRefresh(handle: SessionHandle) {
  const clear = useCallback(() => clearSession(handle), [handle]);
  const refresh = useCallback(() => refreshSession(handle, clear), [handle, clear]);
  return { clear, refresh };
}

/** A renewed token schedules its own next refresh, including near-expiry tokens. */
function useRefreshSchedule(token: string | null, refresh: () => Promise<string | null>, handle: SessionHandle) {
  useEffect(() => (token ? scheduleRefresh(handle, token, refresh) : undefined), [token, refresh, handle]);
}

/** Restores the session once on load, then marks loading done. */
function useRestore(handle: SessionHandle, refresh: () => Promise<string | null>, clear: () => void) {
  useEffect(() => startRestore(handle, refresh, clear), [handle, refresh, clear]);
}

/** Creates the account, then signs in to it. */
async function signUp(email: string, password: string, displayName: string | undefined, login: SignIn) {
  await apiSignup(email, password, displayName);
  await login(email, password);
}

/** Signs in with email and password. */
type SignIn = (email: string, password: string) => Promise<void>;

/** Login, signup, logout and profile adoption. */
function useAccountActions(handle: SessionHandle, clear: () => void) {
  const login = useCallback((email: string, password: string) => signIn(handle, email, password), [handle]);
  const signup = useCallback(
    (email: string, password: string, displayName?: string) => signUp(email, password, displayName, login),
    [login],
  );
  // The cookie is httpOnly; only the server can clear it.
  const logout = useCallback(() => void (clear(), apiLogout()), [clear]);
  const applyProfile = useCallback((profile: User) => handle.setUser(profile), [handle]);
  return { login, signup, logout, applyProfile };
}

/** Everything the auth context provides. */
export function useAuthSession(): AuthContextType {
  const { user, token, loading, handle } = useSessionHandle();
  const { clear, refresh } = useRefresh(handle);
  useRefreshSchedule(token, refresh, handle);
  useRestore(handle, refresh, clear);
  return { user, token, loading, ...useAccountActions(handle, clear) };
}
