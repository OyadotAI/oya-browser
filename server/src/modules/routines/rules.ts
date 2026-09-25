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

/** Daily at a 24-hour HH:MM, the desktop's local time. */
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
  /** What the agent is asked each run. */
  prompt: string;
  /** When it runs. */
  schedule: Schedule;
  /** Off: kept, but never run on schedule. */
  enabled: boolean;
  /** When its last run started (ms), which the next one counts from; null before the first. */
  lastRunAt: number | null;
  /** Its runs, newest first. */
  runs: Run[];
}

/** The fields a client sets on a routine. */
export type Definition = Pick<Routine, 'name' | 'prompt' | 'schedule' | 'enabled'>;

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
    prompt: has('prompt') ? text(input?.prompt, ROUTINE_MAX_PROMPT, 'prompt') : current!.prompt,
    schedule: has('schedule') ? scheduleFrom(input?.schedule) : current!.schedule,
    enabled: input?.enabled === undefined ? (current?.enabled ?? true) : input.enabled === true,
  };
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
