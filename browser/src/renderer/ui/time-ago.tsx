/**
 * A past time in words ("3 days ago"), with the exact date and time on hover
 * and for assistive technology.
 */
import { ago } from './ago.ts';

/** What a TimeAgo shows. */
export interface TimeAgoProps {
  /** When, in epoch milliseconds. */
  at: number;
}

/** A <time> reading how long ago `at` was. */
export function TimeAgo({ at }: TimeAgoProps) {
  const date = new Date(at);
  return (
    <time dateTime={date.toISOString()} title={date.toLocaleString()}>
      {ago(at)}
    </time>
  );
}
