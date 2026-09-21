/**
 * The switcher's popover: a heading with back and close, the failure alert,
 * and whichever panel is open.
 */
'use client';

import { ArrowLeft, KeyRound, X } from 'lucide-react';
import { close, show } from './actions';
import KeyPanel from './key-panel';
import { onPickerKeyDown } from './keyboard';
import ManagePanel from './manage-panel';
import ProjectForm from './project-form';
import ProjectList from './project-list';
import type { PanelProps, SubmitForm } from './types';
import { titleOf } from './view';

/** The heading row: back (inside a panel), title, project count (on the list), close. */
function Heading({ c }: PanelProps) {
  const { form, busy } = c.ui;
  return (
    <div className="flex min-h-14 items-center gap-2 border-b border-border px-4">
      {form && (
        <button
          disabled={busy}
          aria-label="Back to projects"
          onClick={() => show(c, null)}
          className="-ml-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-md text-text-muted hover:bg-text/5"
        >
          <ArrowLeft className="h-4 w-4" />
        </button>
      )}
      <h2 className="min-w-0 flex-1 truncate text-sm font-semibold text-text">{titleOf(c.ui)}</h2>
      {!form && <span className="font-mono text-xs text-text-dim">{c.projects.length}</span>}
      <button
        aria-label="Close projects"
        onClick={() => close(c)}
        className="-mr-1 flex h-8 w-8 items-center justify-center rounded-md text-text-dim hover:bg-text/5 hover:text-text"
      >
        <X className="h-4 w-4" />
      </button>
    </div>
  );
}

/** The last failure, with a restore button when the key can be repaired. */
function Alert({ c }: PanelProps) {
  const { error, restore } = c.ui;
  if (!error) return null;
  return (
    <div role="alert" className="mx-4 mt-3 rounded-lg bg-red-500/8 px-3 py-2.5 text-xs leading-relaxed text-red-400">
      {error}
      {restore && (
        <button
          onClick={() => show(c, 'import', restore)}
          className="mt-2 flex items-center gap-1.5 font-medium text-red-300 underline-offset-2 hover:underline"
        >
          <KeyRound className="h-3.5 w-3.5" />
          Restore with API key
        </button>
      )}
    </div>
  );
}

/** The popover dialog. */
export default function PickerPanel({ c }: PanelProps) {
  const { form, target, revealedKey, busy } = c.ui;
  return (
    <div
      id="project-picker"
      role="dialog"
      aria-label={titleOf(c.ui)}
      aria-busy={busy}
      onKeyDown={(e) => onPickerKeyDown(c, e)}
      className="fixed left-3 right-3 top-[58px] z-50 max-h-[calc(100dvh-76px)] overflow-y-auto rounded-xl border border-border bg-bg-card shadow-[0_16px_48px_-12px_rgba(0,0,0,0.45)] sm:absolute sm:left-auto sm:right-0 sm:top-full sm:mt-2 sm:w-[360px]"
    >
      <Heading c={c} />
      <Alert c={c} />
      {!form && <ProjectList c={c} />}
      {form === 'manage' && target && <ManagePanel c={c} target={target} />}
      {form === 'key' && revealedKey && <KeyPanel c={c} />}
      {form && form !== 'manage' && form !== 'key' && <ProjectForm c={c} form={form as SubmitForm} />}
    </div>
  );
}
