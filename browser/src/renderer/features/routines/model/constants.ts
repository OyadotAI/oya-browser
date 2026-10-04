/**
 * The Routines pane's fixed words and values: what the cards, the history and
 * the editor say, and a new routine's schedule. Times (the clock, how long a
 * note stays) come from core/constants.ts.
 */

/** The words the pane shows. */
export const ROUTINES_TEXT = {
  empty: 'No routines yet',
  emptyHint: 'Run a prompt every hour, or every day at a set time. Routines belong to this project.',
  offline: 'Offline. Connect to Oya to see and run this project’s routines.',
  loadError: (why: string) => `Could not reach this project’s routines: ${why}`,
  off: 'Off',
  dueNow: 'Due now',
  waiting: (why: string) => `Waiting: ${why}`,
  next: (when: string) => `Next ${when}`,
  runningHere: (elapsed: string) => `Running on this browser · ${elapsed}`,
  runningElsewhere: 'Running on another Oya browser',
  runningCloud: 'Running in the cloud',
  runNow: 'Run now',
  stop: 'Stop',
  history: (n: number) => (n ? `History · ${n}` : 'History'),
  more: (name: string) => `More for ${name}`,
  toggle: (name: string) => `Run “${name}” on its schedule`,
  edit: 'Edit',
  clear: 'Clear history',
  remove: 'Delete',
  confirm: (name: string) => `Delete “${name}” and its history?`,
  cancel: 'Cancel',
  every: (n: number, unit: string) => `Every ${n === 1 ? unit.replace(/s$/, '') : `${n} ${unit}`}`,
  daily: (at: string) => `Daily at ${at}`,
  cloud: 'Cloud',
  cloudHint: 'Runs in the cloud',
} as const;

/** The words the run history shows. */
export const RUNS_TEXT = {
  status: {
    running: 'Running',
    done: 'Done',
    failed: 'Failed',
    stopped: 'Stopped',
    interrupted: 'Interrupted',
  } as Readonly<Record<string, string>>,
  never: 'Never run',
  none: 'No runs yet.',
  /** What an open run says when it has no answer, by how it ended. */
  noAnswer: {
    running: 'Working on it…',
    stopped: 'Stopped before it answered.',
    interrupted: 'The browser running it closed before it finished.',
    done: 'No answer.',
    failed: 'No answer.',
  } as Readonly<Record<string, string>>,
  steps: (n: number) => `${n} ${n === 1 ? 'step' : 'steps'}`,
  elsewhere: 'another browser',
} as const;

/** The words the editor shows. */
export const EDITOR_TEXT = {
  cloud: 'Run in the cloud',
  cloudHint: 'Oya runs it on a cloud browser, even when this app is closed.',
  newTitle: 'New routine',
  editTitle: 'Edit routine',
  create: 'Save routine',
  update: 'Save changes',
} as const;

/** The editor's field sizes (match the server's limits on a name and a prompt). */
export const EDITOR_LIMITS = { nameChars: 100, promptChars: 10_000, promptRows: 3 } as const;

/** A new routine's schedule until one is picked (matches the editor's first paint). */
export const DEFAULT_SCHEDULE = { kind: 'every', n: 1, unit: 'hours', at: '09:00' } as const;

/** What Electron puts before an error thrown in the main process. */
export const REMOTE_ERROR_PREFIX = /^Error invoking remote method '[^']+': (Error: )?/;

/** Seconds in a minute, for durations. */
export const SECONDS_PER_MINUTE = 60;
