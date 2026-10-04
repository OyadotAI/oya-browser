/**
 * Validation run records: starting one, recovering those a crash left
 * running, pruning old ones, and folding worker messages into the record.
 */
import { randomUUID } from 'node:crypto';
import { normalizeDraft, generate, type Draft } from '../../workflow/index.ts';
import { WORKSPACE } from './constants.ts';

/** Statuses of a run that is still going. */
const ACTIVE = ['starting', 'running', 'paused', 'stopping'];

/** One event a run reported: a step's progress, evidence, or something needing attention. */
export interface RunEvent {
  /** 'step', 'evidence', 'attention', ... */
  kind?: string;
  /** The step's status, for a step event. */
  status?: string;
  /** What to tell the person. */
  message?: string;
  /** When it happened, in epoch milliseconds. */
  at?: number;
  /** Anything else the worker reported. */
  [field: string]: unknown;
}

/** A repair the worker made, listed on its run. */
export interface Repair {
  /** The repaired step. */
  stepId?: string;
  /** The target it had. */
  original?: unknown;
  /** The target it got. */
  replacement?: unknown;
  /** The new draft the repair was saved as. */
  draftId: string;
}

/** One validation run's record. */
export interface Run {
  /** The run's id. */
  id: string;
  /** The draft it runs. */
  draftId: string;
  /** The draft's revision when it started. */
  revision: number;
  /** A copy of the draft as it started. */
  draft?: Draft;
  /** The exact module it runs. */
  code?: string;
  /** 'starting', 'running', 'paused', 'stopping', 'succeeded', 'failed', 'interrupted', ... */
  status: string;
  /** Why it failed, for the person. */
  error?: string;
  /** When it started, in epoch milliseconds. */
  startedAt: number;
  /** When it finished, in epoch milliseconds. */
  finishedAt?: number;
  /** What it reported, oldest first, bounded. */
  events: RunEvent[];
  /** The repairs it made. */
  repairs: Repair[];
  /** Anything else the finished message carried. */
  [field: string]: unknown;
}

/** A message from the validation worker. */
export interface WorkerMessage {
  /** 'event', 'repair' or 'finished'. */
  type: string;
  /** The event, for an event message. */
  event?: RunEvent;
  /** The repaired draft, for a repair. */
  draft?: Partial<Draft>;
  /** The repaired step, for a repair. */
  stepId?: string;
  /** The target the step had, for a repair. */
  original?: unknown;
  /** The target it got, for a repair. */
  replacement?: unknown;
  /** How the run ended, for finished. */
  status?: string;
  /** Why it failed, for finished. */
  error?: string;
  /** Anything else the worker sent. */
  [field: string]: unknown;
}

/** One stored entry as a store lists it, or a recovery entry for one it cannot read. */
export interface StoredSummary {
  /** The entry's id. */
  id: string;
  /** Its name. */
  name: string;
  /** When it last changed, in epoch milliseconds. */
  updatedAt?: number;
  /** How many steps it has. */
  steps?: number;
  /** Set when it cannot be read. */
  error?: boolean;
}

/** What the workspace keeps drafts and runs in (DraftStore's interface). */
export interface Store {
  /** Every entry, newest first. */
  list(): StoredSummary[];
  /** One entry; throws when it cannot be read. */
  load(id: string): Draft;
  /** Saves an entry and returns what was saved. */
  save(raw: object): unknown;
  /** An entry's size on disk, in bytes. */
  size(id: string): number;
  /** Deletes an entry. */
  remove(id: string): void;
}

/** What folding a worker message reads and changes on the workspace. */
export interface RunHost {
  /** The run being folded into. */
  run: Run | null;
  /** The draft being validated. */
  draft: Draft;
  /** Where a repair is saved. */
  store: Store;
  /** The last storage failure, shown to the person. */
  storageError: string | null;
}

/** Something that carries a run: a stored run record, or a workspace with a run going. */
export interface WithRun {
  /** The run. */
  run: Run;
}

/** A RunHost whose run is known to exist. */
type Running = RunHost & WithRun;

/** A stored run record: draft-shaped, with the run inside. */
type RunRecord = Draft & WithRun;

/** Whether a run is still going. */
export const isActive = (run: Pick<Run, 'status'> | null | undefined): boolean => ACTIVE.includes(String(run?.status));

/** A fresh run record for the draft, with the exact code it will run. */
export function newRun(draft: Draft): Run {
  return {
    ...{ id: randomUUID(), draftId: draft.id, revision: draft.revision, draft: structuredClone(draft) },
    ...{ code: generate(draft).code, status: 'starting', startedAt: Date.now(), events: [], repairs: [] },
  };
}

/** Marks a run Oya closed during as interrupted, and saves it. */
function interrupt(runStore: Store, record: RunRecord): void {
  record.run.status = 'interrupted';
  record.run.error =
    'Oya closed during validation. Check the website before retrying; nothing was automatically resubmitted.';
  runStore.save(record);
}

/** Interrupts runs a previous session left going; returns the latest run of `draftId`, if any. */
export function recoverRuns(runStore: Store | undefined, draftId: string): Run | null {
  let latest: Run | null = null;
  for (const item of runStore?.list() || []) {
    if (item.error) continue;
    const record = runStore!.load(item.id) as RunRecord;
    if (isActive(record.run)) interrupt(runStore!, record);
    if (!latest && record.run?.draftId === draftId) latest = record.run;
  }
  return latest;
}

/** Whether a stored run is past the count, age or total-size limit. */
const expired = (index: number, item: StoredSummary, total: number): boolean =>
  index >= WORKSPACE.MAX_RUNS ||
  Date.now() - (item.updatedAt ?? 0) > WORKSPACE.RUN_MAX_AGE_MS ||
  total > WORKSPACE.RUN_STORAGE_BYTES;

/** Deletes stored runs past the limits, newest kept first. */
export function pruneRuns(runStore: Store | undefined): void {
  let total = 0;
  for (const [index, item] of (runStore?.list() || []).entries()) {
    if (item.error) continue;
    total += runStore!.size(item.id);
    if (expired(index, item, total)) runStore!.remove(item.id);
  }
}

/** A run event: keep it (bounded) and follow pause and resume. */
function onEvent(workspace: Running, message: WorkerMessage): void {
  const { run } = workspace;
  const event = message.event ?? {};
  run.events.push(event);
  if (run.events.length > WORKSPACE.MAX_RUN_EVENTS) run.events.shift();
  if (event.status === 'paused') run.status = 'paused';
  if (event.status === 'running') run.status = 'running';
}

/** The repaired draft: a new, paused copy named after the original. */
function repairedDraft(draft: Draft, message: WorkerMessage): Draft {
  const name = draft.name.slice(0, WORKSPACE.REPAIR_NAME_LENGTH) + '-repair';
  return normalizeDraft({ ...message.draft, id: randomUUID(), name, repairedFrom: draft.id, phase: 'paused' });
}

/** Saves a repair draft; without secure storage, the run says so instead. */
function saveRepair(workspace: Running, repaired: Draft): void {
  try {
    workspace.store.save(repaired);
  } catch (e) {
    workspace.storageError = (e as Error).message;
    const note = 'Repair could not be saved because secure storage is unavailable.';
    workspace.run.events.push({ kind: 'attention', message: note, at: Date.now() });
  }
}

/** A repair: save it as a new draft beside the original and list it on the run. */
function onRepair(workspace: Running, message: WorkerMessage): void {
  const repaired = repairedDraft(workspace.draft, message);
  saveRepair(workspace, repaired);
  const { stepId, original, replacement } = message;
  workspace.run.repairs.push({ stepId, original, replacement, draftId: repaired.id });
}

/** Worker message type → how it changes the run record. */
const RECEIVE: Record<string, (workspace: Running, message: WorkerMessage) => unknown> = {
  event: onEvent,
  repair: onRepair,
  finished: (workspace, message) => Object.assign(workspace.run, message, { finishedAt: Date.now() }),
};

/** Folds a worker message into the workspace's run; unknown types change nothing. */
export function applyMessage(workspace: Running, message: WorkerMessage): void {
  if (Object.hasOwn(RECEIVE, message.type)) RECEIVE[message.type](workspace, message);
}
