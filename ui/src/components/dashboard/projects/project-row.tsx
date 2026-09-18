/**
 * One project on the switcher's list: pick it, or open its options if you own it.
 */
'use client';

import { Check, Loader2, MoreHorizontal } from 'lucide-react';
import { select, show } from './actions';
import { BADGE_LETTERS } from './constants';
import type { PickerContext, Project } from './types';

/** The row's project and the switcher it belongs to. */
interface Props {
  /** The switcher's context. */
  c: PickerContext;
  /** The project on this row. */
  project: Project;
}

/** A selectable project, marked while opening and when it is the open one. */
export default function ProjectRow({ c, project }: Props) {
  const current = c.currentId === project.id,
    busy = c.ui.busy;
  return (
    <div className={`flex items-center rounded-lg transition-colors ${current ? 'bg-accent/8' : 'hover:bg-text/4'}`}>
      <button
        data-project-option
        disabled={busy}
        onClick={() => void select(c, project)}
        aria-current={current}
        className="flex min-w-0 flex-1 items-center gap-3 rounded-lg py-3 pl-3 pr-2 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/50 disabled:cursor-wait"
      >
        <span
          aria-hidden
          className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border text-xs font-semibold ${current ? 'border-accent/20 bg-accent/10 text-accent' : 'border-border bg-text/3 text-text-muted'}`}
        >
          {project.name.slice(0, BADGE_LETTERS).toUpperCase()}
        </span>
        <span className="min-w-0 flex-1 truncate text-sm font-medium text-text">{project.name}</span>
        {c.ui.pending === project.id ? (
          <Loader2 aria-label="Opening" className="h-4 w-4 shrink-0 animate-spin text-text-dim" />
        ) : (
          current && <Check aria-label="Selected" className="h-4 w-4 shrink-0 text-accent" />
        )}
        {!project.owner && <span className="text-[11px] text-text-dim">{project.role}</span>}
      </button>
      {project.owner && (
        <button
          disabled={busy}
          aria-label={`Options for ${project.name}`}
          title="Project options"
          onClick={() => show(c, 'manage', project)}
          className="mr-1 flex h-9 w-9 shrink-0 items-center justify-center rounded-md text-text-dim hover:bg-text/8 hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/50"
        >
          <MoreHorizontal className="h-4 w-4" />
        </button>
      )}
    </div>
  );
}
