/**
 * The question the claim page asks before anything is sent: put this key on
 * my account, or not. It names the key and says what claiming gives the agent
 * holding it, because the link may have come from someone else.
 */
'use client';

import type { FormEvent } from 'react';
import { FormError, SubmitButton } from '@/components/auth/fields';
import type { ClaimState } from './use-claim';

/** Confirm's props. */
interface ConfirmProps {
  /** Where the claim stands: asking, or claiming. */
  state: ClaimState;
  /** Claims the key. */
  onConfirm: () => void;
  /** Forgets the key. */
  onDiscard: () => void;
}

/** The claim question, its warning, and the two answers. */
export function Confirm({ state, onConfirm, onDiscard }: ConfirmProps) {
  const busy = state.status === 'claiming';
  const submit = (e: FormEvent) => {
    e.preventDefault();
    onConfirm();
  };
  return (
    <form onSubmit={submit} className="space-y-4">
      <h1 className="font-display text-2xl font-bold text-text">Claim this agent key?</h1>
      <p className="text-sm text-text-muted">
        Key <span className="font-mono text-text">{state.hint}</span>
      </p>
      <p className="text-sm text-text-muted">
        Only claim a key from an agent you started yourself. The agent holding this key keeps it and keeps access to the
        project it joins on your account, including starting cloud browsers. If someone else sent you this link, discard
        it.
      </p>
      <FormError error={state.error ?? ''} />
      <SubmitButton busy={busy} label="Claim this key" busyLabel="Claiming…" />
      {!busy && (
        <button type="button" onClick={onDiscard} className="text-sm font-medium text-accent hover:text-accent-hover">
          Discard this link
        </button>
      )}
    </form>
  );
}
