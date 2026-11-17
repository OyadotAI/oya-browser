/**
 * A playbook's run history: the newest finished runs, kept in one sealed row per
 * project and playbook in the playbook_runs table, so every replica, the console
 * and the desktop app read the same history after the in-memory run is gone.
 * A version on the row guards each write, as routines do, so two replicas finishing
 * runs at once both land. Only finished runs are kept: a run that is still going
 * lives in runs.ts on the replica running it.
 */
import { getConnection } from '../../platform/storage/index.ts';
import { sealText, openText } from '../../platform/secrets.ts';
import { RUN_ERROR_MAX, RUN_HISTORY_KEPT, RUN_HISTORY_TRIES } from './constants.ts';

/** The table's name. */
const TABLE = 'playbook_runs';

/** One finished run. */
export interface RunRecord {
  /** How it ended. */
  status: 'succeeded' | 'failed';
  /** When it started, as an ISO time. */
  startedAt: string;
  /** When it ended, as an ISO time. */
  endedAt: string;
  /** The browser it ran on. */
  browserId: string;
  /** Why it failed, cut to RUN_ERROR_MAX characters. */
  error?: string;
}

/** A history as read, with the version a write must match. */
interface Stored {
  /** Newest first. */
  runs: RunRecord[];
  /** Its row's version. */
  version: number;
}

/** The sealing scope, so one project's history cannot be opened as another's. */
const scopeOf = (owner: string) => `playbook-runs:${owner}`;

/** A row as a history; one that no longer unseals reads as empty. */
function fromRow(owner: string, row): Stored {
  try {
    return { runs: openText(scopeOf(owner), row.value) as RunRecord[], version: Number(row.version) };
  } catch {
    return { runs: [], version: Number(row.version) };
  }
}

/** The stored history of one playbook, or null when it never ran. */
async function read(owner: string, playbook: string): Promise<Stored | null> {
  const [row] = await getConnection().select(TABLE, { owner, playbook });
  return row ? fromRow(owner, row) : null;
}

/**
 * Writes a whole history as a row at `version`. `replace` overwrites a row already
 * there; without it, a row another replica just created is left alone.
 */
async function put(owner: string, playbook: string, runs: RunRecord[], version: number, replace = false) {
  const row = { owner, playbook, value: sealText(scopeOf(owner), runs), version };
  await getConnection().upsert(TABLE, [{ ...row, updated_at: new Date().toISOString() }], { update: replace });
}

/**
 * The history to add to, creating an empty one at version 0 if there is none. Every
 * run then lands through swap(), so two replicas writing a first run at once both
 * land: the one that loses the version check reads again and retries.
 */
async function readOrCreate(owner: string, playbook: string): Promise<Stored> {
  const current = await read(owner, playbook);
  if (current) return current;
  await put(owner, playbook, [], 0);
  return (await read(owner, playbook)) ?? { runs: [], version: 0 };
}

/** Replaces a history only if it is still at `version`; false when another write got there first. */
async function swap(owner: string, playbook: string, runs: RunRecord[], version: number) {
  const set = { value: sealText(scopeOf(owner), runs), version: version + 1, updated_at: new Date().toISOString() };
  return (await getConnection().update(TABLE, { owner, playbook, version }, set)) > 0;
}

/** The run as kept: its error cut short. */
const kept = (run: RunRecord): RunRecord => (run.error ? { ...run, error: run.error.slice(0, RUN_ERROR_MAX) } : run);

/** One attempt to put `run` at the head of the history; false when another write won the race. */
async function prepend(owner: string, playbook: string, run: RunRecord): Promise<boolean> {
  const current = await readOrCreate(owner, playbook);
  const runs = [kept(run), ...current.runs].slice(0, RUN_HISTORY_KEPT);
  return swap(owner, playbook, runs, current.version);
}

/** Adds a finished run to the playbook's history, retrying a write another replica beat. */
export async function record(owner: string, playbook: string, run: RunRecord): Promise<void> {
  for (let i = 0; i < RUN_HISTORY_TRIES; i++) if (await prepend(owner, playbook, run)) return;
}

/** The playbook's finished runs, newest first. */
export async function runsOf(owner: string, playbook: string): Promise<RunRecord[]> {
  return (await read(owner, playbook))?.runs ?? [];
}

/** Each playbook's newest run, by name, in one read, for the list's last-run dot. */
export async function lastRuns(owner: string): Promise<Map<string, RunRecord>> {
  const rows = await getConnection().select(TABLE, { owner });
  const latest = rows.map((row) => [String(row.playbook), fromRow(owner, row).runs[0]] as const);
  return new Map(latest.filter(([, run]) => run));
}

/** Drops a playbook's history, so a new playbook given its name starts clean. */
export async function forget(owner: string, playbook: string): Promise<void> {
  await getConnection().delete(TABLE, { owner, playbook });
}

/** Moves a playbook's history to its new name, replacing any left there before. */
export async function move(owner: string, from: string, to: string): Promise<void> {
  const current = await read(owner, from);
  if (current) await put(owner, to, current.runs, current.version + 1, true);
  await forget(owner, from);
}
