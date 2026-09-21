/**
 * The playbook table and, once loaded and empty, the note on how to make one.
 */
'use client';

import { Workflow } from 'lucide-react';
import SyntaxCode from '@/components/ui/syntax-code';
import PlaybookRow, { type RowHandlers } from './playbook-row';
import { RECORD_SNIPPET } from './constants';
import type { PlaybookInfo } from './types';

/** Props for the table. */
interface Props {
  /** The playbooks; null until the first load. */
  playbooks: PlaybookInfo[] | null;
  /** The clock the ages are measured from. */
  now: number;
  /** What the row buttons do. */
  on: RowHandlers;
}

/** The column headings. */
function TableHead() {
  return (
    <thead className="sticky top-0 z-10 bg-bg-elevated">
      <tr className="border-b border-border text-left text-[11px] font-medium uppercase tracking-[0.1em] text-text-muted">
        <th className="w-[22%] px-4 py-3">Playbook</th>
        <th className="px-3 py-3">Variables</th>
        <th className="w-[70px] px-2 py-3 text-right">Steps</th>
        <th className="w-[28%] px-3 py-3">Healed draft</th>
        <th className="w-[90px] px-2 py-3 text-right">Created</th>
        <th className="w-[150px] px-2 py-3" aria-label="Actions" />
      </tr>
    </thead>
  );
}

/** No playbooks yet: record one, or save one from code. */
function EmptyPlaybooks() {
  return (
    <div className="flex flex-col items-center justify-center gap-3 px-6 py-24 text-center">
      <div className="flex h-12 w-12 items-center justify-center rounded-xl border border-border bg-bg-card">
        <Workflow className="h-5 w-5 text-text-muted" />
      </div>
      <p className="text-[15px] font-medium text-text">No playbooks yet</p>
      <p className="max-w-md text-[13px] text-text-muted">
        Record one yourself with the button above, or run a task with ask() and save it from code:
      </p>
      <pre className="max-w-full overflow-x-auto rounded-lg border border-border bg-bg-card px-4 py-3 text-left text-[12px]">
        <SyntaxCode code={RECORD_SNIPPET} language="typescript" />
      </pre>
    </div>
  );
}

/** Every playbook as a row. */
export default function PlaybookTable({ playbooks, now, on }: Props) {
  const list = playbooks || [];
  return (
    <div className="data-scroll mx-4 mb-4 min-h-0 flex-initial overflow-auto rounded-xl border border-border bg-bg-card/25 lg:mx-6 lg:mb-6">
      <table className="data-table w-full table-fixed border-collapse text-[13px]" aria-label="Playbooks">
        <TableHead />
        <tbody>
          {list.map((p) => (
            <PlaybookRow key={p.name} p={p} now={now} on={on} />
          ))}
        </tbody>
      </table>
      {playbooks && list.length === 0 && <EmptyPlaybooks />}
    </div>
  );
}
