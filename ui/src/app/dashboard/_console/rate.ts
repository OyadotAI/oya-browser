/**
 * The fleet strip's throughput: commands per minute and error percentage,
 * from the difference between two fleet polls.
 */
import { MS_PER_MINUTE, PERCENT } from './constants';

/** One poll's counters. */
export interface RateSample {
  /** When it was taken (ms). */
  at: number;
  /** Commands run so far. */
  commands: number;
  /** Commands failed so far. */
  errors: number;
}

/** Throughput between two polls. */
export interface Rate {
  /** Commands per minute. */
  commandsPerMin: number;
  /** Failed commands as a percentage of all commands. */
  errorPct: number;
}

/** The rate between two samples, or null without an earlier one. Counters that went back count as zero. */
export function commandRate(prev: RateSample | null, cur: RateSample): Rate | null {
  if (!prev || cur.at <= prev.at) return null;
  const dc = Math.max(0, cur.commands - prev.commands),
    de = Math.max(0, cur.errors - prev.errors);
  const mins = (cur.at - prev.at) / MS_PER_MINUTE;
  return { commandsPerMin: Math.round(dc / mins), errorPct: dc ? (de / dc) * PERCENT : 0 };
}
