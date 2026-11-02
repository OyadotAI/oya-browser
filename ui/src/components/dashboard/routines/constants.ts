/**
 * The Routines tab's numbers and fixed words, copied from the desktop's Routines
 * pane (browser/src/renderer/features/routines/model/constants.ts) so the two say
 * the same; the console adds the lines a desktop never needs.
 */

/** How often the routine list refreshes. */
export const ROUTINES_POLL_MS = 15_000;
/**
 * How long a run counts as going after it started: the server's lease
 * (ROUTINE_RUN_LEASE_MS). A run its server died during stays marked running;
 * past this it is over, as the server treats it, so Run now opens again.
 */
export const RUN_LEASE_MS = 21_600_000;
/** How long a row's Run now note stays. */
export const NOTE_MS = 6000;
/** The most units an "every" schedule may count. */
export const EVERY_MAX = 999;
/** Milliseconds in a second, for durations. */
export const MS_PER_SECOND = 1000;
/** Seconds in a minute, for durations. */
export const SECONDS_PER_MINUTE = 60;

/** The words the tab shows. */
export const ROUTINES_TEXT = {
  title: 'Routines',
  empty: 'No routines yet',
  hint: 'Run a prompt or a saved playbook every hour, or every day at a set time. Routines belong to this project.',
  loadError: (why: string) => `Could not reach this project’s routines: ${why}`,
  create: 'New routine',
  off: 'Off',
  dueNow: 'Due now',
  runningCloud: 'Running in the cloud',
  runningDesktop: 'Running on a desktop',
  runNow: 'Run now',
  starting: 'Starting…',
  alreadyRunning: 'Already running',
  started: 'Started.',
  history: (n: number) => (n ? `History · ${n}` : 'History'),
  toggle: (name: string) => `Run “${name}” on its schedule`,
  edit: (name: string) => `Edit ${name}`,
  remove: (name: string) => `Delete ${name}`,
  clear: 'Clear history',
  confirm: (name: string) => `Delete “${name}” and its history?`,
  confirmBody: 'Its runs go with it. This cannot be undone.',
  deleted: (name: string) => `${name} deleted`,
  gone: 'This routine was deleted elsewhere.',
  every: (n: number, unit: string) => `Every ${n === 1 ? unit.replace(/s$/, '') : `${n} ${unit}`}`,
  daily: (at: string) => `Daily at ${at}`,
  cloud: 'Cloud',
  desktop: 'Desktop',
} as const;

/** The words the run history shows. */
export const RUNS_TEXT = {
  title: 'Runs',
  status: {
    running: 'Running',
    done: 'Done',
    failed: 'Failed',
    stopped: 'Stopped',
    interrupted: 'Interrupted',
  } as Readonly<Record<string, string>>,
  never: 'Never run',
  none: 'No runs yet.',
  /** What a run says when it has no answer, by how it ended. */
  noAnswer: {
    running: 'Working on it…',
    stopped: 'Stopped before it answered.',
    interrupted: 'The browser running it closed before it finished.',
    done: 'No answer.',
    failed: 'No answer.',
  } as Readonly<Record<string, string>>,
  steps: (n: number) => `${n} ${n === 1 ? 'step' : 'steps'}`,
} as const;

/** The words the editor shows. */
export const EDITOR_TEXT = {
  newTitle: 'New routine',
  editTitle: 'Edit routine',
  name: 'Name',
  namePlaceholder: 'Morning inbox',
  task: 'Does',
  askPrompt: 'Ask the agent a prompt',
  playGroup: 'Play a saved playbook',
  playbooksError: (why: string) => `Could not load playbooks: ${why}`,
  prompt: 'Prompt',
  promptPlaceholder: 'Open Gmail and summarize unread emails from today',
  repeat: 'Repeat',
  every: 'Every',
  dailyAt: 'Daily at',
  howOften: 'How often',
  unit: 'Unit',
  timeOfDay: 'Time of day',
  inZone: (tz: string) => `In your time zone (${tz}).`,
  cloud: 'Run in the cloud',
  cloudHint: 'Oya runs it on a cloud browser, even when no desktop is open.',
  desktopHint: 'Runs on an Oya desktop signed into this project, while it is open.',
  enabled: 'Run it on its schedule',
  cancel: 'Cancel',
  create: 'Save routine',
  update: 'Save changes',
  saving: 'Saving…',
  needName: 'Give the routine a name.',
  needPrompt: 'Write the prompt, or pick a playbook.',
  badEvery: `Every 1 to ${EVERY_MAX} minutes or hours.`,
  values: 'Values',
  valuesHint: 'Leave a value blank to use the playbook’s own.',
  secret: 'Uses the playbook’s saved value',
  profile: 'Profile',
  defaultProfile: 'Default',
  profileHint: 'A desktop runs it only while it is on this profile.',
  profileCloudHint: 'The cloud browser runs as this profile.',
} as const;

/** The editor's field sizes (the server's limits on a name and a prompt). */
export const EDITOR_LIMITS = { nameChars: 100, promptChars: 10_000, promptRows: 3 } as const;

/** A new routine's schedule fields until one is picked. */
export const DEFAULT_SCHEDULE = { kind: 'every', n: 1, unit: 'hours', at: '09:00' } as const;

/** The badge colour, by run status; Never run is dim. */
export const BADGE_CLASS: Readonly<Record<string, string>> = {
  running: 'text-accent',
  done: 'text-accent',
  failed: 'text-red',
  stopped: 'text-yellow',
  interrupted: 'text-yellow',
};
