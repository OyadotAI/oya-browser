/** Customer lookup and safe refreshes that cannot replace a newer selection. */
import { useRef, useState, type MutableRefObject } from 'react';
import type { AdminState } from './use-admin';
import type { Found } from './types';

/** What the last search found, or why it found nothing. */
interface Outcome {
  /** Who was found. */ found: Found | null;
  /** Why not. */ error: string;
  /** Identifies the selection that produced this result. */ version: number;
}

/** One lookup with the selection it is permitted to update. */
type Query = {
  /** Authenticated lookup. */ lookup: AdminState['lookup'];
  /** Email to search. */ email: string;
  /** Selection at dispatch. */ version: number;
  /** Latest requested selection. */ latest: MutableRefObject<number>;
  /** Applies the current result. */ keep: (o: Outcome) => void;
};

/** Drops stale successes and failures alike. */
function keepCurrent(q: Query, found: Found | null, error = '') {
  if (q.version === q.latest.current) q.keep({ found, error, version: q.version });
}

/** Runs only a query that still belongs to the selected customer. */
function current(q: Query) {
  if (!q.email || q.version !== q.latest.current) return Promise.resolve();
  return q.lookup(q.email.trim()).then(
    (found) => keepCurrent(q, found),
    (e) => keepCurrent(q, null, e instanceof Error ? e.message : 'No account with that email'),
  );
}

/** Search generations and a refresh callback bound to the displayed result. */
function useResults(lookup: AdminState['lookup']) {
  const [outcome, keep] = useState<Outcome>({ found: null, error: '', version: 0 });
  const latest = useRef(0);
  const search = (email: string) => current({ lookup, email, version: ++latest.current, latest, keep });
  const refresh = () =>
    current({ lookup, email: outcome.found?.profile.email || '', version: outcome.version, latest, keep });
  return { search, refresh, ...outcome };
}

/** The search input and the selected customer's results. */
export function useLookup(lookup: AdminState['lookup']) {
  const [email, setEmail] = useState('');
  const results = useResults(lookup);
  const search = (asked = email) => {
    setEmail(asked);
    return results.search(asked);
  };
  return { ...results, email, setEmail, search };
}

/** The lookup's state, shared by the search box and customer panel. */
export type LookupState = ReturnType<typeof useLookup>;
