/**
 * The routines as the main process sends them to the pane: the snapshot of
 * src/main/routines/routines.ts (`Routines.snapshot()`), with the run fields
 * the server records (when it started and finished, its answer, its steps).
 * Declared here, as the pane reads them; the main process owns the shapes.
 */

/** When a routine runs: "every N units" (`n`, `unit`) or "daily at HH:MM" (`at`). */
export interface Schedule {
  /** 'every' or 'daily'. */
  kind: string;
  /** How many units apart an 'every' routine runs. */
  n?: number;
  /** 'minutes' or 'hours'. */
  unit?: string;
  /** A daily routine's HH:MM. */
  at?: string;
}

/** One run of a routine, as the server records it. */
export interface RoutineRun {
  /** The run's id. */
  id: string;
  /** 'running', 'done', 'failed', 'stopped' or 'interrupted'. */
  status: string;
  /** The browser that ran it. */
  by?: string;
  /** When it started, in epoch milliseconds. */
  startedAt: number;
  /** When it finished, in epoch milliseconds; absent while it runs. */
  finishedAt?: number | null;
  /** What the agent answered, as Markdown. */
  result?: string;
  /** The tools the agent called, in order. */
  steps?: string[];
}

/** A routine with when it next runs, as the pane shows it. */
export interface Routine {
  /** The routine's id. */
  id: string;
  /** Its name in the pane. */
  name?: string;
  /** What the agent is asked. */
  prompt?: string;
  /** Whether its schedule is on. */
  enabled: boolean;
  /** When it runs. */
  schedule?: Schedule;
  /** Its recent runs, newest first. */
  runs?: RoutineRun[];
  /** When it is next due, or null when off or unknown. */
  nextRunAt?: number | null;
  /** Where it runs: on a desktop, or in the cloud. */
  target?: 'desktop' | 'cloud';
}

/** What the Routines pane shows (`RoutinesSnapshot` in the main process). */
export interface RoutinesSnapshot {
  /** The routines, each with its next run. */
  routines: Routine[];
  /** The routine this app is running, or null. */
  running: string | null;
  /** This browser's id, to tell its runs from other desktops'. */
  browserId?: string;
  /** Whether the server can be called. */
  online: boolean;
  /** Why the list could not be read or changed last, or ''. */
  error?: string;
  /** Why a routine cannot run now, or ''. */
  busy: string;
}

/** What a routine is doing: running here or elsewhere, off, due, or scheduled. */
export type Phase = 'here' | 'elsewhere' | 'off' | 'due' | 'scheduled';
