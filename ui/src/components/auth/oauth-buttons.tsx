/**
 * "Continue with Google / GitHub". The server sends the browser to the
 * provider through Supabase, and /auth/callback finishes the sign-in.
 */
'use client';

import { oauthStartUrl } from '@/lib/api';

/** The providers offered, with their button labels. */
const PROVIDERS: Array<['google' | 'github', string]> = [
  ['google', 'Continue with Google'],
  ['github', 'Continue with GitHub'],
];

/** GitHub's mark; lucide-react 1.x dropped brand icons. */
const GITHUB_MARK =
  'M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z';

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
          {provider === 'github' ? (
            <svg viewBox="0 0 16 16" className="h-4 w-4" fill="currentColor" aria-hidden="true">
              <path d={GITHUB_MARK} />
            </svg>
          ) : (
            <span className="font-bold">G</span>
          )}
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
