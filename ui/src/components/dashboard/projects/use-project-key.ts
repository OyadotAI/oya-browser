/**
 * The open project's API key on demand, for Settings and onboarding: fetched
 * with the account token only when asked (it is never listed), copied when the
 * clipboard allows, and kept in component memory only.
 */
import { useState } from 'react';
import { useAuth } from '@/components/auth-provider';
import { Status } from '@/lib/http-status';
import { message, projectRequest } from './project-api';
import type { Failure, KeyAnswer } from './types';

/** What the key field shows. */
export interface KeyState {
  /** The key once fetched, else empty. */
  key: string;
  /** Whether it reached the clipboard. */
  copied: boolean;
  /** Why it could not be fetched, else empty. */
  error: string;
  /** Whether a fetch is in flight. */
  busy: boolean;
}

/** Nothing fetched yet. */
const EMPTY: KeyState = { key: '', copied: false, error: '', busy: false };

/** What a refusal tells the person: only the owner can see a project's key. */
const reason = (e: unknown) =>
  (e as Failure).status === Status.FORBIDDEN
    ? 'Only the project owner can see its API key.'
    : message(e, 'Could not load the API key');

/** Copies `text`; false when the browser refuses, so the field is there to copy by hand. */
async function copy(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

/** Fetches the key and copies it, reporting each step through `set`. */
async function reveal(token: string, projectId: string, set: (s: KeyState) => void) {
  set({ ...EMPTY, busy: true });
  try {
    const { key } = await projectRequest<KeyAnswer>(token, projectId, 'POST', {}, '/key');
    set({ ...EMPTY, key, copied: await copy(key) });
  } catch (e) {
    set({ ...EMPTY, error: reason(e) });
  }
}

/** The key's state and the action that shows and copies it. */
export function useProjectKey(projectId: string | null) {
  const { token } = useAuth();
  const [state, setState] = useState<KeyState>(EMPTY);
  const ready = !!token && !!projectId;
  return { ...state, ready, reveal: () => (ready ? reveal(token, projectId, setState) : Promise.resolve()) };
}
