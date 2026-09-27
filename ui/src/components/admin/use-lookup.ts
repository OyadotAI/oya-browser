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

/** Everything the lookup shows and does. */
export function useLookup(lookup: AdminState['lookup']) {
  const [email, setEmail] = useState('');
  const [outcome, setOutcome] = useState<Outcome>({ found: null, error: '' });
  const search = () =>
    lookup(email.trim()).then(
      (found) => setOutcome({ found, error: '' }),
      (e) => setOutcome(failed(e)),
    );
  return { email, setEmail, search, ...outcome };
}
