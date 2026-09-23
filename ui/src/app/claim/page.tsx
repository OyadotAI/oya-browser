/**
 * Where a person lands from the claim link their AI agent gave them: sign in
 * (or sign up) if needed, and the agent's key joins their account, which
 * unlocks cloud browsers for it. The agent keeps using the same key.
 */
'use client';

import Link from 'next/link';
import type { ReactNode } from 'react';
import { AuthLoading, AuthShell } from '@/components/auth/auth-shell';
import { useClaim, type ClaimState } from './use-claim';

/** A link styled like the auth pages' links. */
const linkClass = 'font-medium text-accent hover:text-accent-hover';

/** What a claimed key unlocks, and the one favor the project asks. */
function Claimed() {
  return (
    <>
      Your agent’s key is now on your account, and it can start Oya Cloud browsers. If Oya helps,{' '}
      <a href="https://github.com/OyadotAI/oya-browser" target="_blank" rel="noopener noreferrer" className={linkClass}>
        a star on GitHub
      </a>{' '}
      goes a long way.
    </>
  );
}

/** The heading and body for each settled state. */
function content(state: ClaimState): [string, ReactNode] {
  if (state.status === 'claimed') return ['Key claimed', <Claimed key="claimed" />];
  if (state.status === 'signed-out')
    return ['Claim your agent’s key', 'Sign in or create an account, and this key joins it.'];
  if (state.status === 'failed')
    return [
      'Could not claim this key',
      <span key="error" role="alert">
        {state.error}
      </span>,
    ];
  return ['No key in this link', 'Ask your agent for its claim_url again.'];
}

/** Footer's props. */
interface FooterProps {
  /** Where the claim stands. */
  state: ClaimState;
}

/** The way on from each settled state. */
function Footer({ state }: FooterProps) {
  if (state.status !== 'signed-out')
    return (
      <Link href="/dashboard" className={linkClass}>
        Go to the dashboard
      </Link>
    );
  return (
    <>
      <Link href="/login" className={linkClass}>
        Sign in
      </Link>{' '}
      or{' '}
      <Link href="/signup" className={linkClass}>
        create an account
      </Link>
    </>
  );
}

/** Claims the key in the link for the signed-in person. */
export default function ClaimPage() {
  const state = useClaim();
  if (state.status === 'claiming') return <AuthLoading />;
  const [title, body] = content(state);
  return (
    <AuthShell glow="accent" footer={<Footer state={state} />}>
      <h1 className="mb-2 font-display text-2xl font-bold text-text">{title}</h1>
      <p className="text-sm text-text-muted">{body}</p>
    </AuthShell>
  );
}
