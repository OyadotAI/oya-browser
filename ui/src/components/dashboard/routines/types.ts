/**
 * A project's routines as the server answers them (server/src/modules/routines/rules.ts,
 * plus the `nextRunAt` the routes add). Declared here because the console cannot
 * import the server's code.
 */

/** When a routine runs: every N minutes or hours, or daily at HH:MM. */
export interface Schedule {
  /** every or daily. */
  kind: 'every' | 'daily';
  /** How many units apart, for every. */
  n?: number;
  /** minutes or hours, for every. */
  unit?: string;
  /** HH:MM, for daily. */
  at?: string;
}

/** One run of a routine. */
export interface RoutineRun {
  /** Its id. */
  id: string;
  /** When it started (ms). */
  startedAt: number;
  /** running, then done, failed, stopped or interrupted. */
  status: string;
  /** The browser that ran it. */
  by?: string;
  /** When it ended (ms). */
  finishedAt?: number;
  /** The agent's answer, or "Error: …". */
  result?: string;
  /** The tools the agent used, in order. */
  steps?: string[];
}

/** A saved routine with its history. */
export interface Routine {
  /** Its id. */
  id: string;
  /** When it was made (ms). */
  createdAt: number;
  /** Its name. */
  name: string;
  /** What it asks the agent; '' when it plays a playbook. */
  prompt: string;
  /** The saved playbook it plays instead of a prompt. */
  playbook?: string;
  /** When it runs. */
  schedule: Schedule;
  /** Whether it runs on its schedule. */
  enabled: boolean;
  /** Where it runs. */
  target: 'desktop' | 'cloud';
  /** The time zone a daily time is read in. */
  tz?: string;
  /** What it sets its playbook's variables to. */
  values?: Record<string, string>;
  /** The persona it runs as; absent for the default. */
  persona?: string | null;
  /** When it last started (ms). */
  lastRunAt: number | null;
  /** When it runs next (ms), worked out by the server; null when off. */
  nextRunAt: number | null;
  /** Its runs, newest first. */
  runs: RoutineRun[];
}

/** What a routine is doing: running, off, due now, or waiting for its next run. */
export type Phase = 'running' | 'off' | 'due' | 'scheduled';

/** What creating or editing a routine sends. */
export interface RoutineBody {
  /** Its name. */
  name: string;
  /** What it asks the agent. */
  prompt: string;
  /** The playbook it plays, or null to ask the prompt again. */
  playbook: string | null;
  /** When it runs. */
  schedule: Schedule;
  /** Whether it runs on its schedule. */
  enabled: boolean;
  /** Where it runs. */
  target: 'desktop' | 'cloud';
  /** The time zone a daily time is read in: the person's own. */
  tz: string;
  /** What it sets its playbook's variables to; only with a playbook. */
  values?: Record<string, string>;
  /** The persona it runs as, or null for the default. */
  persona: string | null;
}
