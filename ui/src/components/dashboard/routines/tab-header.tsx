/**
 * The Routines tab's title, count, what routines are for, and New routine.
 */
'use client';

import { Plus } from 'lucide-react';
import { ROUTINES_TEXT } from './constants';

/** Props for the header. */
interface Props {
  /** How many routines there are. */
  count: number;
  /** Opens the editor on a new routine. */
  onCreate: () => void;
}

/** The title, count and New routine. */
export default function TabHeader({ count, onCreate }: Props) {
  return (
    <div className="flex shrink-0 flex-wrap items-center gap-3 px-4 py-5 lg:px-6">
      <div className="mr-auto">
        <h2 className="text-[22px] font-medium tracking-tight text-text">
          {ROUTINES_TEXT.title} <span className="ml-2 text-[14px] text-text-dim">{count}</span>
        </h2>
        <p className="text-[12px] text-text-muted">{ROUTINES_TEXT.hint}</p>
      </div>
      <button className="btn-primary" onClick={onCreate}>
        <Plus className="h-4 w-4" /> {ROUTINES_TEXT.create}
      </button>
    </div>
  );
}
