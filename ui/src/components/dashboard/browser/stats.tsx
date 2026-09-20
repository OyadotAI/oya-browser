/**
 * The browser's numbers: status, uptime, last seen, commands, errors, in flight,
 * and the last error if there was one.
 */
import { ago } from '@/lib/api-client';
import type { BrowserDetail } from '../types';
import { HEALTH_LABEL } from '../types';

/** Text colour per tone; anything else is plain text. */
const TONE: Record<string, string> = {
  ok: 'text-accent',
  errors: 'text-red',
  stale: 'text-yellow',
  dead: 'text-text-muted',
};

/** The colour class for a tone. */
export const toneClass = (tone?: string) => (tone && Object.hasOwn(TONE, tone) ? TONE[tone] : 'text-text');

/** One labelled number. */
interface StatProps {
  /** Label. */
  k: string;
  /** Value, already formatted. */
  v: string;
  /** Colours the value: a health, or undefined for plain. */
  tone?: string;
}

/** A label over a value. */
function Stat({ k, v, tone }: StatProps) {
  return (
    <div className="min-w-0">
      <dt className="text-[10.5px] uppercase tracking-[0.12em] text-text-dim">{k}</dt>
      <dd className={`truncate num ${toneClass(tone)}`}>{v}</dd>
    </div>
  );
}

/** The six stats in a grid, then the last error. */
export default function Stats({
  d,
  now,
}: {
  /** The browser. */ d: BrowserDetail;
  /** Clock for the times. */ now: number;
}) {
  return (
    <>
      <dl className="grid grid-cols-3 gap-x-4 gap-y-2 rounded-md border border-border bg-bg px-3 py-2 text-[12.5px]">
        <Stat k="Status" v={HEALTH_LABEL[d.health]} tone={d.health} />
        <Stat k="Uptime" v={ago(d.connectedAt, now)} />
        <Stat k="Last seen" v={ago(d.lastSeen, now)} />
        <Stat k="Commands" v={String(d.commands)} />
        <Stat k="Errors" v={String(d.errors)} tone={d.errors ? 'errors' : undefined} />
        <Stat k="In flight" v={String(d.pending)} />
      </dl>
      {d.lastError && (
        <p className="truncate text-[12px] text-red" title={d.lastError}>
          Last error: {d.lastError}
        </p>
      )}
    </>
  );
}
