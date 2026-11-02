/**
 * The routine table and, once loaded and empty, what a routine is for. A row's
 * history opens in a second row under it, kept open across refreshes.
 */
'use client';

import { CalendarClock } from 'lucide-react';
import { Fragment, useState } from 'react';
import RoutineRow from './routine-row';
import RoutineRuns from './routine-runs';
import { ROUTINES_TEXT } from './constants';
import type { RoutineActions } from './use-routine-actions';
import type { Routine } from './types';

/** Props for the table. */
interface Props {
  /** The routines; null until the first load. */
  routines: Routine[] | null;
  /** The clock times are worded against. */
  now: number;
  /** What the rows' buttons do. */
  actions: RoutineActions;
  /** Opens the editor on a routine. */
  onEdit: (r: Routine) => void;
}

/** The column headings. */
function TableHead() {
  const th = 'px-3 py-3';
  return (
    <thead className="sticky top-0 z-10 bg-bg-elevated">
      <tr className="border-b border-border text-left text-[11px] font-medium uppercase tracking-[0.1em] text-text-muted">
        <th className="w-[22%] px-4 py-3">Routine</th>
        <th className={`w-[130px] ${th}`}>Schedule</th>
        <th className={`w-[80px] ${th}`}>Where</th>
        <th className={`w-[60px] ${th}`}>On</th>
        <th className={`w-[140px] ${th}`}>Last run</th>
        <th className={`w-[150px] ${th}`}>Next run</th>
        <th className="w-[230px] px-2 py-3" aria-label="Actions" />
      </tr>
    </thead>
  );
}

/** No routines yet: what one does. */
function EmptyRoutines() {
  return (
    <div className="flex flex-col items-center justify-center gap-3 px-6 py-24 text-center">
      <div className="flex h-12 w-12 items-center justify-center rounded-xl border border-border bg-bg-card">
        <CalendarClock className="h-5 w-5 text-text-muted" />
      </div>
      <p className="text-[15px] font-medium text-text">{ROUTINES_TEXT.empty}</p>
      <p className="max-w-md text-[13px] text-text-muted">{ROUTINES_TEXT.hint}</p>
    </div>
  );
}

/** Every routine as a row, each with its history when opened. */
export default function RoutineTable({ routines, now, actions, onEdit }: Props) {
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const list = routines || [];
  return (
    <div className="data-scroll mx-4 mb-4 min-h-0 flex-initial overflow-auto rounded-xl border border-border bg-bg-card/25 lg:mx-6 lg:mb-6">
      <table className="data-table routine-data w-full table-fixed border-collapse text-[13px]" aria-label="Routines">
        <TableHead />
        <tbody>
          {list.map((r) => (
            <Fragment key={r.id}>
              <RoutineRow
                routine={r}
                now={now}
                actions={actions}
                open={Boolean(open[r.id])}
                onHistory={() => setOpen((o) => ({ ...o, [r.id]: !o[r.id] }))}
                onEdit={() => onEdit(r)}
              />
              {open[r.id] && (
                <tr id={`runs-${r.id}`}>
                  <td colSpan={7} className="border-b border-border/60 bg-bg/40 px-4 py-3">
                    <RoutineRuns routine={r} now={now} onClear={() => actions.clear(r)} />
                  </td>
                </tr>
              )}
            </Fragment>
          ))}
        </tbody>
      </table>
      {routines && list.length === 0 && <EmptyRoutines />}
    </div>
  );
}
