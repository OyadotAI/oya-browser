/**
 * Recorded steps name their tab ('main', 'tab-1', …) rather than carry the
 * desktop's tab ids, which mean nothing to a replay.
 */
import type { RecordedStep } from './types.ts';

/** Tab id → recorded tab name. */
export class RecordingTabNames {
  /** The recording's steps, asked when a name is chosen. */
  private readonly steps: () => RecordedStep[];
  /** Tab id → name. */
  private readonly names = new Map<number | null, string>();

  /** `steps()` is the recording's steps, whose names are taken too. */
  constructor(steps: () => RecordedStep[]) {
    this.steps = steps;
  }

  /** The tab's name, choosing the first free `tab-N` for a new one. */
  recordingTab(id: number | null): string {
    const name = this.names.get(id) ?? this.freeName();
    this.names.set(id, name);
    return name;
  }

  /** The first `tab-N` no tab and no step uses. */
  private freeName(): string {
    const used = new Set([...this.names.values(), ...this.steps().map((s) => s.tab)]);
    let n = 1;
    while (used.has('tab-' + n)) n++;
    return 'tab-' + n;
  }

  /** Names a tab explicitly. */
  set(id: number | null, name: string): void {
    this.names.set(id, name);
  }

  /** How many tabs are named. */
  get size(): number {
    return this.names.size;
  }

  /** Forgets every name. */
  clear(): void {
    this.names.clear();
  }
}
