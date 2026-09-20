/**
 * The sign-in session for the whole app: AuthProvider holds it, useAuth()
 * reads it. The session logic lives in lib/auth.
 */
'use client';

import { createContext, useContext, type PropsWithChildren } from 'react';
import { useAuthSession } from '@/lib/auth/use-auth-session';
import type { AuthContextType } from '@/lib/auth/types';

/** The session, or null outside a provider. */
const AuthContext = createContext<AuthContextType | null>(null);

/** Provides the session to everything below it. */
export function AuthProvider({ children }: PropsWithChildren) {
  const session = useAuthSession();
  return <AuthContext.Provider value={session}>{children}</AuthContext.Provider>;
}

/** The session; throws outside AuthProvider so a missing provider fails loudly. */
export function useAuth() {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error('useAuth must be used within AuthProvider');
  return ctx;
}
