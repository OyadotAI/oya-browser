/**
 * "Continue with Google / GitHub". The server sends the browser to the
 * provider through Supabase, and /auth/callback finishes the sign-in.
 */
'use client';

import { Github } from 'lucide-react';
import { oauthStartUrl } from '@/lib/api';

/** The providers offered, with their button labels. */
const PROVIDERS: Array<['google' | 'github', string]> = [
  ['google', 'Continue with Google'],
  ['github', 'Continue with GitHub'],
];

/** Leaves for `provider`'s sign-in, asking it to come back to this console. */
function start(provider: 'google' | 'github') {
  window.location.assign(oauthStartUrl(provider, `${window.location.origin}/auth/callback`));
}

/** The provider buttons and the divider before the email form. */
export function OAuthButtons() {
  return (
    <div className="mb-6 space-y-3">
      {PROVIDERS.map(([provider, label]) => (
        <button
          key={provider}
          type="button"
          onClick={() => start(provider)}
          className="flex w-full items-center justify-center gap-2 rounded-lg border border-border px-4 py-2.5 text-sm font-medium text-text hover:bg-bg-input"
        >
          {provider === 'github' ? <Github className="h-4 w-4" /> : <span className="font-bold">G</span>}
          {label}
        </button>
      ))}
      <div className="flex items-center gap-3 pt-2 text-xs text-text-dim">
        <span className="h-px flex-1 bg-border" />
        or with email
        <span className="h-px flex-1 bg-border" />
      </div>
    </div>
  );
}
