/**
 * Numbers for routines: how often the scheduler looks, what each schedule
 * unit is worth, and how much of a run its history keeps.
 */

/** How often the routines scheduler looks for a routine that is due (one minute, its finest schedule). */
export const ROUTINE_TICK_MS = 60_000;

/** Milliseconds in each unit an "every N" routine may repeat in. */
export const ROUTINE_UNIT_MS = { minutes: 60_000, hours: 3_600_000 } as const;

/** How much of a run's answer a routine's history keeps. */
export const ROUTINE_RESULT_CHARS = 4000;

/** How many of a run's steps (tool names) its history keeps. */
export const ROUTINE_STEPS_KEPT = 100;
