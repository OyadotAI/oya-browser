/**
 * Holds the project session in React state: which project is open and what
 * the account can open, with startup and renewal wired to effects.
 */
import { useEffect, useMemo, useState } from 'react';
import { startup, watchRenewal } from './session';
import type { Counter, Listing, Session, Toast } from './types';

/** The session object, rebuilt only when the token, credential setter or toast change. */
function useSession(token: string | null, setApiKey: Session['setApiKey'], toast: Toast) {
  const [currentId, setCurrentId] = useState<string | null>(null);
  const [listing, setListing] = useState<Listing>({ projects: [], keys: [] });
  // A box that lives as long as the component; the session's steps bump it outside render.
  const [opening] = useState<Counter>(() => ({ current: 0 }));
  const session = useMemo<Session>(
    () => ({ token, setApiKey, toast, opening, setCurrentId, setListing }),
    [token, setApiKey, toast, opening],
  );
  return { session, currentId, listing };
}

/** The open project, the listing, and the session the switcher's actions act on. */
export function useProjectSession(token: string | null, setApiKey: Session['setApiKey'], toast: Toast) {
  const { session, currentId, listing } = useSession(token, setApiKey, toast);
  useEffect(() => startup(session), [session]);
  useEffect(() => (currentId ? watchRenewal(session, currentId) : undefined), [session, currentId]);
  return { session, currentId, ...listing };
}
