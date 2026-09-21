/**
 * A project's API key, shown once to be copied.
 */
'use client';

import { Check, Copy } from 'lucide-react';
import { copyRevealed } from './actions';
import { FIELD_CLASS } from './constants';
import type { PanelProps } from './types';

/** The key in a read-only field, with a copy button. */
export default function KeyPanel({ c }: PanelProps) {
  const { revealedKey, copied } = c.ui;
  return (
    <div className="space-y-4 p-5">
      <p className="text-xs leading-relaxed text-text-muted">
        Use this key to connect your tools and browsers. Keep it private.
      </p>
      <label className="block space-y-2">
        <span className="text-xs font-medium text-text">API key</span>
        <input
          autoFocus
          readOnly
          value={revealedKey}
          onFocus={(e) => e.target.select()}
          className={`${FIELD_CLASS} font-mono`}
        />
      </label>
      <button
        className="flex h-10 w-full items-center justify-center gap-2 rounded-lg bg-accent text-sm font-medium text-accent-foreground hover:bg-accent-hover"
        onClick={() => void copyRevealed(c)}
      >
        {copied ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
        {copied ? 'Copied to clipboard' : 'Copy API key'}
      </button>
    </div>
  );
}
