/**
 * One row of the playbook table: name, variables, steps, a healed draft waiting
 * for review, age, and the row's actions.
 */
'use client';

import { Code, Pencil, Play, Trash2 } from 'lucide-react';
import { ago } from '@/lib/api-client';
import { DRAFT_SUFFIX } from './constants';
import type { PlaybookBody, PlaybookInfo } from './types';

/** What a row's buttons open or do. */
export interface RowHandlers {
  /** Shows a playbook's (or draft's) Playwright code. */
  onCode: (body: PlaybookBody) => void;
  /** Opens the rename dialog. */
  onRename: (p: PlaybookInfo) => void;
  /** Opens the run dialog. */
  onRun: (p: PlaybookInfo) => void;
  /** Asks to delete a playbook, or `name:draft` for only its draft. */
  onRemove: (name: string) => void;
  /** Makes the healed draft the playbook's steps. */
  onPromote: (name: string) => void;
}

/** Props for a row. */
interface RowProps {
  /** The playbook shown. */
  p: PlaybookInfo;
  /** The clock the ages are measured from. */
  now: number;
  /** What the buttons do. */
  on: RowHandlers;
}

/** Props for the variables cell. */
interface VariablesProps {
  /** The playbook's variable names. */
  variables: string[];
}

/** The variables as chips, or "none". */
function VariablesCell({ variables }: VariablesProps) {
  return (
    <td className="px-3 py-3">
      <div className="flex flex-wrap gap-1">
        {variables.length ? (
          variables.map((v) => (
            <span
              key={v}
              className="rounded border border-border px-1.5 py-0.5 font-mono text-[11px] text-text-secondary"
            >
              {v}
            </span>
          ))
        ) : (
          <span className="text-text-dim">none</span>
        )}
      </div>
    </td>
  );
}

/** The healed draft, if any, with review, promote and discard. */
function DraftCell({ p, now, on }: RowProps) {
  if (!p.draft)
    return (
      <td className="px-3 py-3">
        <span className="text-text-dim">—</span>
      </td>
    );
  const draft = p.draft;
  return (
    <td className="px-3 py-3">
      <div className="flex flex-wrap items-center gap-2">
        <span
          className="text-[12px] text-yellow"
          title={`The replay broke at step ${draft.healedFrom + 1}; the agent finished it.`}
        >
          {draft.steps} steps · {ago(draft.healedAt, now)}
        </span>
        <button className="btn-ghost h-7 px-2 text-[12px]" onClick={() => on.onCode(draft)}>
          Review
        </button>
        <button className="btn-primary h-7 px-2 text-[12px]" onClick={() => on.onPromote(p.name)}>
          Promote
        </button>
        <button className="btn-ghost h-7 px-2 text-[12px]" onClick={() => on.onRemove(`${p.name}${DRAFT_SUFFIX}`)}>
          Discard
        </button>
      </div>
    </td>
  );
}

/** Code, rename, run and delete. */
function ActionsCell({ p, on }: Omit<RowProps, 'now'>) {
  return (
    <td className="px-2 py-3">
      <div className="flex justify-end gap-1">
        <button
          className="btn-icon"
          title="Playwright code"
          aria-label={`Playwright code for ${p.name}`}
          onClick={() => on.onCode(p)}
        >
          <Code className="h-4 w-4" />
        </button>
        <button className="btn-icon" title="Rename" aria-label={`Rename ${p.name}`} onClick={() => on.onRename(p)}>
          <Pencil className="h-4 w-4" />
        </button>
        <button className="btn-icon" title="Run" aria-label={`Run ${p.name}`} onClick={() => on.onRun(p)}>
          <Play className="h-4 w-4" />
        </button>
        <button className="btn-icon" title="Delete" aria-label={`Delete ${p.name}`} onClick={() => on.onRemove(p.name)}>
          <Trash2 className="h-4 w-4" />
        </button>
      </div>
    </td>
  );
}

/** One playbook as a table row. */
export default function PlaybookRow({ p, now, on }: RowProps) {
  return (
    <tr className="border-b border-border/60 hover:bg-text/[0.035]">
      <td className="px-2 py-3 pl-4">
        <div className="truncate font-medium text-text" title={p.name}>
          {p.name}
        </div>
        {p.promotedAt && <div className="mt-1 text-[11px] text-text-dim">healed {ago(p.promotedAt, now)} ago</div>}
      </td>
      <VariablesCell variables={p.variables} />
      <td className="px-2 py-3 text-right num text-text-secondary">{p.steps}</td>
      <DraftCell p={p} now={now} on={on} />
      <td className="px-2 py-3 text-right num text-text-muted">{ago(p.createdAt, now)}</td>
      <ActionsCell p={p} on={on} />
    </tr>
  );
}
