/**
 * The row above a snippet: the language tabs, and the show-key and copy buttons.
 */
'use client';

import { Copy, Check, Eye, EyeOff } from 'lucide-react';
import type { Snippet } from './types';

/** What the toolbar shows and does. */
interface Props {
  /** Every tab. */
  snippets: Snippet[];
  /** The open tab's id. */
  activeId: string;
  /** Whether the real key is shown. */
  reveal: boolean;
  /** Whether a copy just succeeded. */
  copied: boolean;
  /** Opens a tab. */
  onChoose: (id: string) => void;
  /** Shows or hides the key. */
  onToggleReveal: () => void;
  /** Copies the open snippet with the real key. */
  onCopy: () => void;
}

/** Tabs on the left, key and copy actions on the right. */
export default function SnippetToolbar({
  snippets,
  activeId,
  reveal,
  copied,
  onChoose,
  onToggleReveal,
  onCopy,
}: Props) {
  return (
    <div className="mb-3 flex flex-wrap items-center gap-2">
      <div
        className="inline-flex max-w-full overflow-x-auto rounded-lg border border-border bg-bg p-1"
        role="tablist"
        aria-label="Snippet language"
      >
        {snippets.map((x) => (
          <button
            key={x.id}
            role="tab"
            aria-selected={activeId === x.id}
            onClick={() => onChoose(x.id)}
            className={`shrink-0 rounded-md px-3 py-1 text-[12.5px] font-medium transition-colors ${activeId === x.id ? 'bg-accent/15 text-text' : 'text-text-muted hover:text-text'}`}
          >
            {x.label}
          </button>
        ))}
      </div>
      <div className="ml-auto flex items-center gap-2">
        <button className="btn-ghost h-7" onClick={onToggleReveal} title={reveal ? 'Hide the key' : 'Show the key'}>
          {reveal ? <EyeOff className="h-3.5 w-3.5" /> : <Eye className="h-3.5 w-3.5" />}{' '}
          {reveal ? 'Hide key' : 'Show key'}
        </button>
        <button className="btn-primary h-7" onClick={onCopy}>
          {copied ? <Check className="h-3.5 w-3.5" /> : <Copy className="h-3.5 w-3.5" />}{' '}
          {copied ? 'Copied' : 'Copy with key'}
        </button>
      </div>
    </div>
  );
}
