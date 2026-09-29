/**
 * The open project's API key as a button that shows and copies it, for places
 * outside the project menu where people look for it: Settings and onboarding.
 */
'use client';

import { Check, Copy, KeyRound, Loader2 } from 'lucide-react';
import { FIELD_CLASS } from './constants';
import { useProjectKey } from './use-project-key';

/** The icon for the button's current state: fetching, copied, shown, or not yet asked for. */
function KeyIcon({
  busy,
  copied,
  shown,
}: {
  /** Fetching. */
  busy: boolean;
  /** Copied. */
  copied: boolean;
  /** The key is on screen. */
  shown: boolean;
}) {
  if (busy) return <Loader2 className="h-4 w-4 animate-spin" />;
  if (copied) return <Check className="h-4 w-4" />;
  return shown ? <Copy className="h-4 w-4" /> : <KeyRound className="h-4 w-4" />;
}

/** The key field (once fetched), the copy button and any refusal. */
export default function ProjectKey({ projectId }: { /** The open project. */ projectId: string | null }) {
  const k = useProjectKey(projectId);
  return (
    <div className="space-y-2">
      {k.key && (
        <input
          readOnly
          aria-label="API key"
          value={k.key}
          onFocus={(e) => e.target.select()}
          className={`${FIELD_CLASS} font-mono`}
        />
      )}
      <button type="button" className="btn-primary" onClick={() => void k.reveal()} disabled={k.busy || !k.ready}>
        <KeyIcon busy={k.busy} copied={k.copied} shown={!!k.key} />
        {k.copied ? 'Copied to clipboard' : k.key ? 'Copy again' : 'Show and copy API key'}
      </button>
      {k.error && (
        <p role="alert" className="text-xs text-red-400">
          {k.error}
        </p>
      )}
    </div>
  );
}
