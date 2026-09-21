/**
 * An owned project's options: copy its API key, rename it, delete it.
 */
'use client';

import { Copy, KeyRound, Loader2, Pencil, Trash2 } from 'lucide-react';
import { copyKey, show } from './actions';
import { ACTION_CLASS } from './constants';
import type { TargetProps } from './types';

/** The key's prefix (with an ellipsis), or the project id when no key is listed. */
function KeyHint({ c, target }: TargetProps) {
  return (
    <p className="px-3 pb-3 pt-2 font-mono text-[11px] text-text-dim">
      {c.keys.find((k) => k.project === target.id)?.prefix || target.id}
      {c.keys.some((k) => k.project === target.id && k.prefix) && '…'}
    </p>
  );
}

/** The options for `target`. */
export default function ManagePanel({ c, target }: TargetProps) {
  const busy = c.ui.busy;
  return (
    <div className="p-2">
      <KeyHint c={c} target={target} />
      <button
        data-management-action
        disabled={busy}
        aria-label={`Copy API key for ${target.name}`}
        onClick={() => void copyKey(c, target)}
        className={`${ACTION_CLASS} text-text-muted hover:text-text`}
      >
        <KeyRound className="h-4 w-4" />
        <span className="flex-1">Copy API key</span>
        {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <Copy className="h-3.5 w-3.5 text-text-dim" />}
      </button>
      <button
        disabled={busy}
        aria-label={`Rename ${target.name}`}
        onClick={() => show(c, 'rename', target)}
        className={`${ACTION_CLASS} text-text-muted hover:text-text`}
      >
        <Pencil className="h-4 w-4" />
        Rename project
      </button>
      <div className="mx-3 my-2 border-t border-border" />
      <button
        disabled={busy}
        aria-label={`Delete ${target.name}`}
        onClick={() => show(c, 'delete', target)}
        className={`${ACTION_CLASS} text-red-400 hover:bg-red-500/8 hover:text-red-400`}
      >
        <Trash2 className="h-4 w-4" />
        Delete project
      </button>
    </div>
  );
}
