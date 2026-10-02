/**
 * The person lookup's state: the email typed, what the server found, and why
 * it found nothing.
 */
import { useState } from 'react';
import type { AdminState } from './use-admin';
import type { Found } from './types';

/** What the last search found, or why it found nothing. */
interface Outcome {
  /** Who was found. */
  found: Found | null;
  /** Why not. */
  error: string;
}

/** A failed search, said as the server said it. */
const failed = (e: unknown): Outcome => ({
  found: null,
  error: e instanceof Error ? e.message : 'No account with that email',
});

/** Keeps what a search answered, or why it failed. */
const settle = (asked: Promise<Found>, keep: (o: Outcome) => void) =>
  asked.then(
    (found) => keep({ found, error: '' }),
    (e) => keep(failed(e)),
  );

/** Everything the lookup shows and does. `search` looks up the typed email, or the one given (which it also types). */
export function useLookup(lookup: AdminState['lookup']) {
  const [email, setEmail] = useState('');
  const [outcome, setOutcome] = useState<Outcome>({ found: null, error: '' });
  const search = (asked = email) => {
    setEmail(asked);
    return settle(lookup(asked.trim()), setOutcome);
  };

  return { email, setEmail, search, ...outcome };
}

/** The lookup's state, as the search box and the customers tab share it. */
export type LookupState = ReturnType<typeof useLookup>;
