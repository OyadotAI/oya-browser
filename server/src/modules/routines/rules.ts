/**
 * What a routine may be: the checks on a definition from a client, on a
 * finished run's record, and on routines imported from a desktop that kept
 * them locally. Every refusal is a 400 that says what is wrong.
 */
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import {
  ROUTINE_END_STATUSES,
  ROUTINE_MAX_EVERY,
  ROUTINE_MAX_NAME,
  ROUTINE_MAX_PROMPT,
  ROUTINE_MAX_VALUES,
  ROUTINE_PERSONA_CHARS,
  ROUTINE_VALUE_CHARS,
  ROUTINE_VALUE_NAME_CHARS,
  ROUTINE_RESULT_CHARS,
  ROUTINE_RUNS_KEPT,
  ROUTINE_STEP_CHARS,
  ROUTINE_STEPS_KEPT,
  ROUTINE_UNITS,
} from './constants.ts';

/** Every N minutes or hours. */
export interface EverySchedule {
  /** The kind. */
  kind: 'every';
  /** How many units apart. */
  n: number;
  /** minutes or hours. */
  unit: string;
}

/** Daily at a 24-hour HH:MM: the desktop's local time, or the routine's `tz` when it runs in the cloud. */
export interface DailySchedule {
  /** The kind. */
  kind: 'daily';
  /** HH:MM. */
  at: string;
}

/** When a routine runs. */
export type Schedule = EverySchedule | DailySchedule;

/** One run of a routine. */
export interface Run {
  /** Its id, chosen by the browser that ran it. */
  id: string;
  /** When it started (ms). */
  startedAt: number;
  /** running, then done, failed, stopped or interrupted. */
  status: string;
  /** The browser running it, so only that one stops it or marks it interrupted. */
  by?: string;
  /** When it ended (ms). */
  finishedAt?: number;
  /** The agent's answer, or "Error: …". */
  result?: string;
  /** The tools the agent used, in order. */
  steps?: string[];
}

/** A saved routine, with its history. */
export interface Routine {
  /** Its id. */
  id: string;
  /** When it was made (ms); an "every" schedule counts from here until the first run. */
  createdAt: number;
  /** Its name. */
  name: string;
  /** What the agent is asked each run; '' when the routine plays a playbook instead. */
  prompt: string;
  /** The saved playbook each run plays, by name, instead of asking a prompt. */
  playbook?: string;
  /**
   * The playbook's variable values each run plays with. A variable left out uses
   * the playbook's saved default, so a secret is never stored here: it stays in
   * the playbook, saved once.
   */
  values?: Record<string, string>;
  /** The persona each run is the browser of; absent for the key's default one. */
  persona?: string;
  /** When it runs. */
  schedule: Schedule;
  /** Off: kept, but never run on schedule. */
  enabled: boolean;
  /** Where it runs: on a project desktop that claims it, or on a cloud browser the server starts. */
  target: Target;
  /** The IANA time zone a daily cloud routine's time is in; a desktop uses its own. */
  tz?: string;
  /** The project a cloud routine runs for, which is how the server finds its key with no caller. */
  project?: string;
  /** When its last run started (ms), which the next one counts from; null before the first. */
  lastRunAt: number | null;
  /** Its runs, newest first. */
  runs: Run[];
}

/** Where a routine runs. */
export type Target = 'desktop' | 'cloud';

/** The fields a client sets on a routine. */
export type Definition = Pick<
  Routine,
  'name' | 'prompt' | 'playbook' | 'values' | 'schedule' | 'enabled' | 'target' | 'tz' | 'persona'
>;

/** A refusal. */
const refuse = (message: string) => new HttpError(Status.BAD_REQUEST, message);

/** A daily time, 24-hour HH:MM. */
const DAILY_TIME = /^([01]\d|2[0-3]):[0-5]\d$/;

/** A trimmed text of 1 to `max` characters, or a refusal naming `field`. */
function text(value: unknown, max: number, field: string): string {
  if (typeof value !== 'string' || !value.trim() || value.length > max)
    throw refuse(`A routine's ${field} is 1 to ${max} characters.`);
  return value.trim();
}

/** Schedule kind → the schedule as stored, or a refusal. */
const SCHEDULES: Record<string, (s: any) => Schedule> = {
  every: ({ n, unit }) => {
    if (!Number.isInteger(n) || n < 1 || n > ROUTINE_MAX_EVERY || !ROUTINE_UNITS.includes(unit))
      throw refuse(`"Every" runs every 1 to ${ROUTINE_MAX_EVERY} minutes or hours.`);
    return { kind: 'every', n, unit };
  },
  daily: ({ at }) => {
    if (typeof at !== 'string' || !DAILY_TIME.test(at)) throw refuse('"Daily" runs at a time written HH:MM.');
    return { kind: 'daily', at };
  },
};

/** Just the fields a schedule of its kind has, or a refusal. */
export function scheduleFrom(input: any): Schedule {
  if (!input || !Object.hasOwn(SCHEDULES, input.kind)) throw refuse('A routine runs "every" N units or "daily".');
  return SCHEDULES[input.kind](input);
}

/** A routine's settings from `input`, each field `input` leaves out taken from `current`. */
export function definitionFrom(input: any, current?: Definition): Definition {
  const has = (field: string) => input?.[field] !== undefined || !current;
  return {
    name: has('name') ? text(input?.name, ROUTINE_MAX_NAME, 'name') : current!.name,
    ...taskFrom(input, current),
    schedule: has('schedule') ? scheduleFrom(input?.schedule) : current!.schedule,
    enabled: input?.enabled === undefined ? (current?.enabled ?? true) : input.enabled === true,
    ...placeFrom(input, current),
  };
}

/** A persona id, or undefined for the key's default one (null or ''); whose it is, the service checks. */
function personaFrom(value: unknown): string | undefined {
  if (value === null || value === '' || value === 'default') return undefined;
  if (typeof value !== 'string' || value.length > ROUTINE_PERSONA_CHARS)
    throw refuse("A routine's profile is a profile id.");
  return value;
}

/** A variable name: what a playbook's {{name}} placeholders use. */
const VALUE_NAME = new RegExp(`^\\w{1,${ROUTINE_VALUE_NAME_CHARS}}$`);

/** The playbook's variable values, bounded, or a refusal; an empty map is none. */
function valuesFrom(value: unknown): Record<string, string> | undefined {
  if (value === null || value === undefined) return undefined;
  if (typeof value !== 'object' || Array.isArray(value)) throw refuse("A routine's values map variable names to text.");
  const entries = Object.entries(value);
  if (entries.length > ROUTINE_MAX_VALUES) throw refuse(`A routine keeps at most ${ROUTINE_MAX_VALUES} values.`);
  for (const [k, v] of entries) checkValue(k, v);
  return entries.length ? Object.fromEntries(entries) : undefined;
}

/** Refuses one value whose name is not a variable name or whose value is not short text. */
function checkValue(name: string, value: unknown) {
  if (!VALUE_NAME.test(name)) throw refuse(`"${name.slice(0, ROUTINE_VALUE_NAME_CHARS)}" is not a variable name.`);
  if (typeof value !== 'string' || value.length > ROUTINE_VALUE_CHARS)
    throw refuse(`A routine's value for ${name} is text of at most ${ROUTINE_VALUE_CHARS} characters.`);
}

/**
 * What each run does: plays a saved playbook, or asks the agent a prompt. A playbook
 * wins; `playbook: null` switches back to the prompt. Each field `input` leaves out is
 * taken from `current`.
 */
function taskFrom(input: any, current?: Definition): Pick<Definition, 'prompt' | 'playbook' | 'values'> {
  const playbook = input?.playbook === undefined ? current?.playbook : playbookFrom(input.playbook);
  const given = input?.prompt === undefined ? current?.prompt : input.prompt;
  if (!playbook) return promptTask(input, given);
  return { playbook, prompt: given ? text(given, ROUTINE_MAX_PROMPT, 'prompt') : '', values: valuesOf(input, current) };
}

/** A routine that asks the agent `given`: it has no playbook, and so no values. */
function promptTask(input: any, given: unknown): Pick<Definition, 'prompt' | 'playbook' | 'values'> {
  if (!given) throw refuse('A routine runs a prompt or a saved playbook: give one of them.');
  if (input?.values && Object.keys(input.values).length)
    throw refuse("A routine's values are for the playbook it plays.");
  // Explicit undefineds: an edit's merge must drop a playbook, and its values, the routine had.
  return { prompt: text(given, ROUTINE_MAX_PROMPT, 'prompt'), playbook: undefined, values: undefined };
}

/** The values a playbook routine keeps: the ones given, else the ones it had while it plays the same playbook. */
const valuesOf = (input: any, current?: Definition) => {
  if (input?.values !== undefined) return valuesFrom(input.values);
  // Another playbook's values would go out as this one's data.
  return input?.playbook === undefined || input.playbook === current?.playbook ? current?.values : undefined;
};

/** A playbook's name, or undefined for none (null or ''). */
function playbookFrom(value: unknown): string | undefined {
  return value === null || value === '' ? undefined : text(value, ROUTINE_MAX_NAME, 'playbook');
}

/** Where and in which time zone the routine runs, each left as `current` has it when `input` leaves it out. */
function placeFrom(input: any, current?: Definition): Pick<Definition, 'target' | 'tz' | 'persona'> {
  const has = (field: string) => input?.[field] !== undefined || !current;
  return {
    target: has('target') ? targetFrom(input?.target) : (current!.target ?? 'desktop'),
    tz: has('tz') ? tzFrom(input?.tz) : current!.tz,
    persona: input?.persona === undefined ? current?.persona : personaFrom(input.persona),
  };
}

/** Where a routine runs: "desktop" when left out, else "desktop" or "cloud". */
function targetFrom(value: unknown): Target {
  if (value === undefined || value === 'desktop' || value === 'cloud') return (value as Target) ?? 'desktop';
  throw refuse('A routine runs on "desktop" or in the "cloud".');
}

/** An IANA time zone the runtime knows, or UTC when left out. */
function tzFrom(value: unknown): string {
  if (value === undefined) return 'UTC';
  try {
    return new Intl.DateTimeFormat('en-US', { timeZone: String(value) }).resolvedOptions().timeZone;
  } catch {
    throw refuse('A routine\'s time zone is an IANA name, like "Europe/London".');
  }
}

/** A finished run's record from a client: its status, answer and steps, bounded. */
export function endingFrom(input: any): Pick<Run, 'status' | 'result' | 'steps' | 'finishedAt'> {
  if (!ROUTINE_END_STATUSES.includes(input?.status))
    throw refuse(`A run ends as one of: ${ROUTINE_END_STATUSES.join(', ')}.`);
  const result = typeof input.result === 'string' ? input.result.slice(0, ROUTINE_RESULT_CHARS) : '';
  const steps = Array.isArray(input.steps) ? input.steps.filter((s) => typeof s === 'string') : [];
  const bounded = steps.slice(0, ROUTINE_STEPS_KEPT).map((s) => s.slice(0, ROUTINE_STEP_CHARS));
  return { status: input.status, result, steps: bounded, finishedAt: Date.now() };
}

/** A past run as a desktop kept it: bounded, and a run it never finished marked interrupted. */
function importedRun(run: any): Run | null {
  if (typeof run?.id !== 'string' || !Number.isFinite(run.startedAt)) return null;
  const status = ROUTINE_END_STATUSES.includes(run.status) ? run.status : 'interrupted';
  const ending = endingFrom({ ...run, status });
  return { id: run.id, startedAt: run.startedAt, ...ending, finishedAt: Number(run.finishedAt) || run.startedAt };
}

/** A routine a desktop kept locally, as the server stores it; its id, creation and history carried over. */
export function importedRoutine(input: any): Routine {
  if (typeof input?.id !== 'string' || !input.id) throw refuse('An imported routine needs its id.');
  const runs = (Array.isArray(input.runs) ? input.runs : []).map(importedRun).filter(Boolean) as Run[];
  const createdAt = Number.isFinite(input.createdAt) ? input.createdAt : Date.now();
  const lastRunAt = Number.isFinite(input.lastRunAt) ? input.lastRunAt : null;
  return { id: input.id, createdAt, ...definitionFrom(input), lastRunAt, runs: runs.slice(0, ROUTINE_RUNS_KEPT) };
}
