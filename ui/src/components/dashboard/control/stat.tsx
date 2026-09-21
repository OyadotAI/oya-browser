/**
 * The Control tab's small display pieces: a headline stat card and a
 * capacity bar.
 */
import type { Activity } from 'lucide-react';
import { TONE_CLASS, barTone, usedPct, type Tone } from './format';

/** What a stat card shows. */
interface StatProps {
  /** Caption above the value. */
  label: string;
  /** The figure itself, already formatted. */
  value: string;
  /** Detail under the value. */
  sub?: string;
  /** Colour of the value. */
  tone?: Tone;
  /** Icon beside the caption. */
  icon?: typeof Activity;
}

/** A headline figure with its caption and detail line. */
export function Stat({ label, value, sub, tone = 'normal', icon: Icon }: StatProps) {
  return (
    <div className="min-w-0 border border-border rounded-xl p-5 bg-bg-card/45">
      <div className="flex items-center gap-2 text-text-muted text-[11px] mb-4">
        {Icon && <Icon className="w-3.5 h-3.5" />}
        {label}
      </div>
      <div className={`tabular-nums text-[30px] font-medium tracking-tight ${TONE_CLASS[tone]}`}>{value}</div>
      {sub && <div className="text-text-dim text-[12px] mt-2">{sub}</div>}
    </div>
  );
}

/** How full a provider is: amber when busy, red when nearly out. */
export function Bar({
  used,
  capacity,
}: {
  /** Slots in use. */ used: number;
  /** Slots in total. */ capacity: number;
}) {
  const pct = usedPct(used, capacity);
  return (
    <div className="h-1.5 bg-border rounded-full overflow-hidden w-full" title={`${used} of ${capacity}`}>
      <div className={`h-full ${barTone(pct)} transition-all`} style={{ width: `${pct}%` }} />
    </div>
  );
}
