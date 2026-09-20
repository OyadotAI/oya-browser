/**
 * The fleet strip's throughput: commands per minute, error rate, and in-flight commands.
 */
import type { Fleet } from '../types';
import { ERROR_RATE_ALERT_PCT, ERROR_RATE_DECIMALS } from './constants';

/** Rates from the last two polls, and the latest counts. */
export interface ThroughputProps {
  /** Command counters from the previous poll, for a per-minute rate. */
  rate: {
    /** Commands in the last minute. */
    commandsPerMin: number;
    /** Share of those that failed, in percent. */
    errorPct: number;
  } | null;
  /** Latest counts, for the in-flight number. */
  b?: Fleet['browsers'];
}

/** "N cmd/min · N% errors · N in flight"; the error rate turns red past the alert level. */
export default function Throughput({ rate, b }: ThroughputProps) {
  const alert = rate && rate.errorPct >= ERROR_RATE_ALERT_PCT;
  return (
    <div className="flex items-center gap-4 text-[12.5px] num text-text-secondary">
      <span>
        <span className="text-text">{rate ? rate.commandsPerMin : '—'}</span> cmd/min
      </span>
      <span>
        <span className={alert ? 'text-red' : 'text-text'}>
          {rate ? `${rate.errorPct.toFixed(ERROR_RATE_DECIMALS)}%` : '—'}
        </span>{' '}
        errors
      </span>
      {b && b.pending > 0 && (
        <span>
          <span className="text-yellow">{b.pending}</span> in flight
        </span>
      )}
    </div>
  );
}
