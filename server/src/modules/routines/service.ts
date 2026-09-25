/**
 * A project's routines: saved prompts its desktop apps run on a schedule. The
 * server keeps them and their history, so every desktop on the project sees
 * the same list; the desktops run them. Before a desktop runs one it claims
 * the run: the claim succeeds for exactly one desktop per due run, because it
 * checks the routine's last run and writes through a versioned row. Every
 * change is told to the project's connected desktops, which re-read the list.
 */
import { randomUUID } from 'node:crypto';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { fingerprint } from '../../platform/audit.ts';
import { registry } from '../browsers/registry.ts';
import * as repository from './repository.ts';
import { definitionFrom, endingFrom, importedRoutine, type Routine, type Run } from './rules.ts';
import { ROUTINE_MAX_PER_KEY, ROUTINE_RUN_LEASE_MS, ROUTINE_RUNS_KEPT, ROUTINE_WRITE_TRIES } from './constants.ts';

export type { Routine, Run } from './rules.ts';

/** Tells the project's desktops that its routines changed. */
const notify = (apiKey: string) => registry.tell(apiKey, { type: 'routines_changed' });

/** The routine, or a 404. */
const missing = () => new HttpError(Status.NOT_FOUND, 'No such routine');

/** Every routine of the project, oldest first. */
export async function list(apiKey: string): Promise<Routine[]> {
  const stored = await repository.all(fingerprint(apiKey));
  return stored.map((s) => s.routine).sort((a, b) => a.createdAt - b.createdAt);
}

/** Adds a routine from a client's definition. */
export async function create(apiKey: string, input: unknown): Promise<Routine> {
  const owner = fingerprint(apiKey);
  if ((await repository.all(owner)).length >= ROUTINE_MAX_PER_KEY)
    throw new HttpError(Status.CONFLICT, `A project keeps at most ${ROUTINE_MAX_PER_KEY} routines.`);
  const routine = { id: randomUUID(), createdAt: Date.now(), ...definitionFrom(input), lastRunAt: null, runs: [] };
  await repository.insert(owner, routine);
  notify(apiKey);
  return routine;
}

/**
 * Applies `change` to a routine and saves it, retrying on a newer version so a
 * concurrent write (another desktop finishing a run) is kept, not overwritten.
 */
async function change(apiKey: string, id: string, edit: (r: Routine) => Routine): Promise<Routine> {
  const owner = fingerprint(apiKey);
  for (let tries = 0; tries < ROUTINE_WRITE_TRIES; tries++) {
    const stored = await repository.one(owner, id);
    if (!stored) throw missing();
    const next = edit(stored.routine);
    if (await repository.swap(owner, next, stored.version)) return (notify(apiKey), next);
  }
  throw new HttpError(Status.CONFLICT, 'The routine changed while saving. Try again.');
}

/** Changes a routine's name, prompt, schedule or on/off; its history is kept. */
export const edit = (apiKey: string, id: string, input: unknown) =>
  change(apiKey, id, (r) => ({ ...r, ...definitionFrom(input, r) }));

/** Deletes a routine and its history. */
export async function remove(apiKey: string, id: string): Promise<void> {
  if (!(await repository.remove(fingerprint(apiKey), id))) throw missing();
  notify(apiKey);
}

/** Drops a routine's finished runs; a run in progress stays. */
export const clearRuns = (apiKey: string, id: string) =>
  change(apiKey, id, (r) => ({ ...r, runs: r.runs.filter((run) => run.status === 'running') }));

/** Whether a run is still going: marked running, and not older than the lease. */
const live = (run: Run, now: number) => run.status === 'running' && now - run.startedAt < ROUTINE_RUN_LEASE_MS;

/** A run the lease ran out on is taken to have died with its browser. */
const expired = (run: Run, now: number): Run =>
  run.status === 'running' && !live(run, now) ? { ...run, status: 'interrupted', finishedAt: now } : run;

/** What a claim asks for: the last run the desktop saw, its run's id, and which browser it is. */
export interface Claim {
  /** The routine's lastRunAt as the desktop read it; the claim fails if it moved since. */
  lastRunAt: number | null;
  /** The new run's id. */
  runId: string;
  /** The browser that will run it. */
  browserId?: string;
}

/** Why a claim is refused, or '' when it can go ahead. */
function claimRefusal(r: Routine, claim: Claim, now: number): string {
  if (r.runs.some((run) => live(run, now))) return 'This routine is already running.';
  if ((r.lastRunAt ?? null) !== (claim.lastRunAt ?? null)) return 'Another browser already ran this routine.';
  return typeof claim.runId === 'string' && claim.runId ? '' : 'A claim needs its run id.';
}

/**
 * Claims the routine's next run for one browser: it starts now, and its next
 * run counts from here. Exactly one of several desktops claiming the same due
 * run gets it; the others get a 409 and let it be.
 */
export async function claim(apiKey: string, id: string, claim: Claim): Promise<Routine> {
  const owner = fingerprint(apiKey);
  const stored = await repository.one(owner, id);
  if (!stored) throw missing();
  const next = claimed(stored.routine, claim, Date.now());
  if (!(await repository.swap(owner, next, stored.version)))
    throw new HttpError(Status.CONFLICT, 'Another browser already ran this routine.');
  return (notify(apiKey), next);
}

/** The routine with `claim`'s run started at `now`, or a 409 saying why it cannot be. */
function claimed(routine: Routine, claim: Claim, now: number): Routine {
  const refusal = claimRefusal(routine, claim, now);
  if (refusal) throw new HttpError(Status.CONFLICT, refusal);
  const run: Run = { id: claim.runId, startedAt: now, status: 'running', by: claim.browserId };
  const runs = [run, ...routine.runs.map((r) => expired(r, now))].slice(0, ROUTINE_RUNS_KEPT);
  return { ...routine, lastRunAt: now, runs };
}

/** Records how a run ended: its status, answer and steps. */
export async function finishRun(apiKey: string, id: string, runId: string, input: unknown): Promise<Routine> {
  const ending = endingFrom(input);
  const finish = (run: Run) => (run.id === runId ? { ...run, ...ending } : run);
  return change(apiKey, id, (r) => {
    if (!r.runs.some((run) => run.id === runId)) throw new HttpError(Status.NOT_FOUND, 'No such run');
    return { ...r, runs: r.runs.map(finish) };
  });
}

/** Takes in routines a desktop kept locally; any already here (same id) are left alone. Answers how many came in. */
export async function importRoutines(apiKey: string, input: unknown): Promise<number> {
  if (!Array.isArray(input)) throw new HttpError(Status.BAD_REQUEST, 'Import a list of routines.');
  const owner = fingerprint(apiKey);
  const have = new Set((await repository.all(owner)).map((s) => s.routine.id));
  const fresh = input.map(importedRoutine).filter((r) => !have.has(r.id));
  const room = Math.max(0, ROUTINE_MAX_PER_KEY - have.size);
  for (const routine of fresh.slice(0, room)) await repository.insert(owner, routine);
  if (fresh.length) notify(apiKey);
  return Math.min(fresh.length, room);
}
