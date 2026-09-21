/**
 * One numbered step of Slack setup, ticked once it is satisfied.
 */
'use client';

import type { ReactNode } from 'react';
import { Check } from 'lucide-react';

/** A step's number, title, state and body. */
interface Props {
  /** Its number. */
  n: number;
  /** What to do. */
  title: string;
  /** Satisfied: show a tick. */
  done: boolean;
  /** Not reachable yet. */
  muted?: boolean;
  /** The step's controls. */
  children: ReactNode;
}

/** The step's card tone: done, unreachable, or open. */
function tone(done: boolean, muted?: boolean) {
  if (done) return 'border-accent/25 bg-accent/[0.04]';
  return muted ? 'border-border bg-bg-sunken/40' : 'border-border bg-bg-sunken/60';
}

/** A numbered card. */
export default function Step({ n, title, done, muted, children }: Props) {
  return (
    <li className={`rounded-lg border p-4 ${tone(done, muted)}`}>
      <div className="mb-3 flex items-center gap-2.5">
        <span
          aria-hidden="true"
          className={`flex h-5 w-5 shrink-0 items-center justify-center rounded-full text-[11px] font-semibold ${done ? 'bg-accent text-bg' : 'bg-text/10 text-text-muted'}`}
        >
          {done ? <Check className="h-3 w-3" /> : n}
        </span>
        <h4 className={`text-[13px] font-medium ${muted ? 'text-text-muted' : 'text-text'}`}>{title}</h4>
      </div>
      {children}
    </li>
  );
}
