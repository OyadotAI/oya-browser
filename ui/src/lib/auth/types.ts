/**
 * The shapes the sign-in session is made of: the person, what the auth
 * context offers, and the handle the session steps share.
 */
import type { Dispatch, RefObject, SetStateAction } from 'react';

/** The signed-in person. */
export interface User {
  /** Account id. */
  id: string;
  /** Sign-in email. */
  email: string;
  /** Name shown in the console. */
  display_name?: string;
  /** Account role, when the server reports one. */
  role?: string;
  /** When the account was created. */
  created_at?: string;
}

/** What useAuth() gives a component. */
export interface AuthContextType {
  /** The signed-in person, or null. */
  user: User | null;
  /** The current access token, or null. */
  token: string | null;
  /** True until the stored session has been restored or ruled out. */
  loading: boolean;
  /** Signs in with email and password. */
  login: (email: string, password: string) => Promise<void>;
  /** Creates an account and signs in to it. */
  signup: (email: string, password: string, displayName?: string) => Promise<void>;
  /** Forgets the session here and on the server. */
  logout: () => void;
  /** Adopt a profile the server just returned, so the UI is not stale until reload. */
  applyProfile: (profile: User) => void;
}

/** The setters and refs every session step shares; stable for the provider's life. */
export interface SessionHandle {
  /** Sets the signed-in person. */
  setUser: Dispatch<SetStateAction<User | null>>;
  /** Sets the access token. */
  setToken: Dispatch<SetStateAction<string | null>>;
  /** Sets whether the session is still being restored. */
  setLoading: Dispatch<SetStateAction<boolean>>;
  /** The scheduled token renewal. */
  timer: RefObject<ReturnType<typeof setTimeout> | null>;
  /** Bumped on every sign-in and sign-out, so a stale refresh cannot overwrite a newer session. */
  version: RefObject<number>;
}
