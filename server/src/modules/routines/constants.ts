/**
 * Every number routines run on, by name: how long a routine's name and prompt
 * may be, how much of each run is kept, and how the run lock behaves.
 */
import { DEFAULT_PORT } from '../../app/constants.ts';

/** The longest routine name, in characters. */
export const ROUTINE_MAX_NAME = 100;
/** The longest routine prompt, in characters. */
export const ROUTINE_MAX_PROMPT = 10_000;
/** The largest N in "every N minutes or hours". */
export const ROUTINE_MAX_EVERY = 999;
/** The most routines one project keeps. */
export const ROUTINE_MAX_PER_KEY = 100;
/** How much of a run's answer the history keeps. */
export const ROUTINE_RESULT_CHARS = 4000;
/** How many runs each routine keeps, newest first; older ones are dropped. */
export const ROUTINE_RUNS_KEPT = 20;
/** How many of a run's steps (tool names) its record keeps. */
export const ROUTINE_STEPS_KEPT = 100;
/** The longest step name kept. */
export const ROUTINE_STEP_CHARS = 64;
/**
 * A run still marked running after this long is taken to have died with its
 * browser, so the routine can be claimed again. Longer than any real run.
 */
export const ROUTINE_RUN_LEASE_MS = 21_600_000; // six hours
/** How many times a change is retried when another writer got there first. */
export const ROUTINE_WRITE_TRIES = 3;
/** The units an "every N" routine repeats in. */
export const ROUTINE_UNITS = ['minutes', 'hours'];
/** How a finished run can end. */
export const ROUTINE_END_STATUSES = ['done', 'failed', 'stopped', 'interrupted'];
/** How long one unit of an "every N" schedule is, in ms. */
export const ROUTINE_UNIT_MS: Record<string, number> = { minutes: 60_000, hours: 3_600_000 };
/** How often the server looks for cloud routines that are due. */
export const ROUTINE_CLOUD_TICK_MS = 60_000;
/** How long a cloud routine's browser has to start and connect before the run fails. */
export const ROUTINE_CLOUD_READY_MS = 180_000;
/** How often the runner checks whether that browser has connected. */
export const ROUTINE_CLOUD_POLL_MS = 3_000;
/**
 * Where the runner reaches this server's own API. The chat request goes through
 * the API rather than a function call so cluster forwarding takes it to whichever
 * replica the cloud browser joined.
 */
export const ROUTINE_SELF_URL = process.env.OYA_SELF_URL || `http://127.0.0.1:${process.env.PORT || DEFAULT_PORT}`;
