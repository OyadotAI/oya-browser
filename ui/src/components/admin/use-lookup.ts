/** Customer selection owns its pending state and rejects results from older selections or credentials. */
import { useRef, useState, type MutableRefObject } from 'react';
import type { AdminScope, AdminState } from './use-admin';
import type { Found } from './types';

/** Standalone callers without authentication changes retain their existing public hook contract. */
const DEFAULT_SCOPE: AdminScope = { current: true, authenticated: true };

/** One selected account's state belongs to one authenticated lifetime. */
interface Outcome {
  /** Customer snapshot. */ found: Found | null;
  /** Visible failure. */ error: string;
  /** Latest selected email, displayed during lookup. */ pending: string;
  /** Selection identity. */ version: number;
  /** Credential lifetime that owns this result. */ scope: AdminScope;
}

/** A dispatched lookup carries revocable selection and authentication ownership. */
type Query = {
  /** Authenticated lookup. */ lookup: AdminState['lookup'];
  /** Selected address. */ email: string;
  /** Selection identity. */ version: number;
  /** Latest selected identity. */ latest: MutableRefObject<number>;
  /** Credential lifetime. */ scope: AdminScope;
  /** Applies visible outcome. */ keep: (value: Outcome) => void;
};

/** Old successes and failures cannot replace a new account or authenticated session. */
function keepCurrent(query: Query, found: Found | null, error = '') {
  if (query.scope.current && query.version === query.latest.current)
    query.keep({ found, error, pending: '', version: query.version, scope: query.scope });
}

/** Pending state is visible immediately, rather than showing the previous customer's actions. */
function current(query: Query) {
  if (!query.email || !query.scope.current || query.version !== query.latest.current) return Promise.resolve();
  return query.lookup(query.email).then(
    (found) => keepCurrent(query, found),
    (error) => keepCurrent(query, null, error instanceof Error ? error.message : 'Could not look up this account.'),
  );
}

/** Selection is published before work starts so previous actions disappear immediately. */
function selected(query: Query) {
  query.keep({
    found: null,
    error: query.email ? '' : 'Enter a customer email.',
    pending: query.email,
    version: query.version,
    scope: query.scope,
  });
  return current(query);
}

/** Selection changes clear the previous account before dispatch; refresh stays bound to its selection. */
function useResults(lookup: AdminState['lookup'], scope: AdminScope) {
  const [outcome, keep] = useState<Outcome>({ found: null, error: '', pending: '', version: 0, scope });
  const latest = useRef(0);
  const query = (email: string, version: number) => ({ lookup, email, version, latest, scope, keep });
  const search = (email: string) => selected(query(email.trim(), ++latest.current));
  const refresh = () => current(query(outcome.found?.profile.email || '', outcome.version));
  return { ...visible(outcome, scope), search, refresh };
}

/** A replacement credential immediately withholds the previous customer's protected data. */
function visible(outcome: Outcome, scope: AdminScope) {
  return outcome.scope === scope ? outcome : { found: null, error: '', pending: '', version: 0, scope };
}

/** Shared search input and customer view use the same selection owner. */
export function useLookup(lookup: AdminState['lookup'], scope = DEFAULT_SCOPE) {
  const [email, setEmail] = useState('');
  const results = useResults(lookup, scope);
  const search = (asked = email) => {
    setEmail(asked);
    return results.search(asked);
  };
  return { ...results, email, setEmail, search };
}

/** Public search and result controls. */
export type LookupState = ReturnType<typeof useLookup>;
