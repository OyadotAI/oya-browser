/**
 * The browser panel's activity log. Its row lives in `browser/activity-row.tsx`.
 */
'use client';

import { useState } from 'react';
import type { Activity } from './types';
import type { OptimisticLine } from './browser/types';
import ActivityRow from './browser/activity-row';

/** The server's log, the optimistic lines, and the clock. */
interface Props {
  /** Commands the server has logged, newest first. */
  items: Activity[];
  /** Things the user just did from the live view, shown until the server echoes them. */
  optimistic: OptimisticLine[];
  /** Clock the ages are measured from. */
  now: number;
}

/** What the browser has been doing, newest first. This is the answer to "what is it doing?". */
export default function ActivityLog({ items, optimistic, now }: Props) {
  const [errorsOnly, setErrorsOnly] = useState(false);
  const rows = errorsOnly ? items.filter((a) => !a.ok) : items;
  return (
    <div className="flex min-h-0 flex-col">
      <div className="mb-1.5 flex items-center justify-between">
        <h3 className="label mb-0">Activity</h3>
        <label className="flex items-center gap-1.5 text-[11.5px] text-text-muted">
          <input
            type="checkbox"
            checked={errorsOnly}
            onChange={(e) => setErrorsOnly(e.target.checked)}
            className="accent-accent"
          />
          errors only
        </label>
      </div>
      <div className="max-h-[260px] overflow-y-auto rounded-md border border-border bg-bg font-mono text-[12px]">
        {optimistic.map((o, i) => (
          <div key={`o-${i}`} className="flex items-center gap-2 border-b border-border/60 px-2 py-1 text-text-dim">
            <span className="w-8 text-right num">…</span>
            <span className="truncate">{o.line}</span>
          </div>
        ))}
        {rows.length === 0 && optimistic.length === 0 && (
          <div className="px-2 py-6 text-center text-text-dim">
            Nothing yet. Click the live view, or send it somewhere.
          </div>
        )}
        {rows.map((a, i) => (
          <ActivityRow key={`${a.ts}-${i}`} a={a} now={now} />
        ))}
      </div>
    </div>
  );
}
