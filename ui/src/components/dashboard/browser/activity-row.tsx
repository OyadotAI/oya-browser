/**
 * One line of the activity log: age, outcome dot, action, summary and duration.
 */
import { ago } from '@/lib/api-client';
import type { Activity } from '../types';

/** The entry and the clock its age is measured from. */
interface Props {
  /** The logged command. */
  a: Activity;
  /** Clock the age is measured from. */
  now: number;
}

/** A logged command; failures are tinted red and carry the error as a tooltip. */
export default function ActivityRow({ a, now }: Props) {
  return (
    <div
      className={`flex items-center gap-2 border-b border-border/60 px-2 py-1 ${a.ok ? '' : 'bg-red/5'}`}
      title={a.error || undefined}
    >
      <span className="w-8 shrink-0 text-right num text-text-dim">{ago(a.ts, now)}</span>
      <span className={`w-1.5 h-1.5 shrink-0 rounded-full ${a.ok ? 'bg-accent' : 'bg-red'}`} />
      <span className="w-[112px] shrink-0 truncate text-text">{a.action}</span>
      <span className="min-w-0 flex-1 truncate text-text-secondary">
        {a.error ? `${a.summary}, ${a.error}` : a.summary}
      </span>
      <span className="shrink-0 num text-text-dim">{a.ms}ms</span>
    </div>
  );
}
