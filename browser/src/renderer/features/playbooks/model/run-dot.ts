/**
 * The dot beside each playbook in the library: whether its last run passed or
 * failed, whether it is running here now or waiting for the person, or that it
 * never ran. No dot at all when the server could not say, so an outage never
 * reads as "never run".
 */
import { ago } from '../../../ui/ago.ts';
import type { Playbook } from './types.ts';

/** What a row's dot shows. */
export type RunDot = 'succeeded' | 'failed' | 'running' | 'attention' | 'never';

/** A row's dot and the words a screen reader hears for it. */
export interface RunDotView {
  /** Which dot. */
  dot: RunDot;
  /** What it means, in words. */
  label: string;
}

/** The run this desktop is following, if it belongs to the row. */
export interface ActiveRun {
  /** The playbook it replays. */
  name: string;
  /** Its server status. */
  status: string;
}

/** A run this desktop is following for this playbook, as a dot. */
function activeDot(active: ActiveRun): RunDotView {
  return active.status === 'needs_attention'
    ? { dot: 'attention', label: 'Needs your help' }
    : { dot: 'running', label: 'Running now' };
}

/** The row's dot, or null when nothing is known about its runs. */
export function runDot(item: Playbook, active: ActiveRun | null, now = Date.now()): RunDotView | null {
  if (active && active.name === item.name) return activeDot(active);
  if (item.lastRun === undefined) return null;
  if (item.lastRun === null) return { dot: 'never', label: 'Never run' };
  const when = ago(Date.parse(item.lastRun.endedAt), now);
  const passed = item.lastRun.status === 'succeeded';
  return { dot: passed ? 'succeeded' : 'failed', label: `Last run ${passed ? 'succeeded' : 'failed'}, ${when}` };
}
