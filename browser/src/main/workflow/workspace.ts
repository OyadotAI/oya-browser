/**
 * The workflow workspace: the draft being edited, its undo history, the local
 * draft library, and validation runs. The main process drives it; every change
 * is saved and published to the renderer as one snapshot.
 *
 * Behind it: edits.ts (the editor command map) and runs.ts (run records,
 * recovery, pruning and worker messages).
 */
import { normalizeDraft, generate, issues, redact, type Draft, type Generated } from '../../workflow/index.ts';
import { WORKSPACE } from './constants.ts';
import { applyEdit, type EditCommand } from './edits.ts';
import {
  isActive,
  newRun,
  recoverRuns,
  pruneRuns,
  applyMessage,
  type Run,
  type Store,
  type WorkerMessage,
  type WithRun,
} from './runs.ts';
import type { RunOptions, ValidationSession } from './validation.ts';

/** Starts validating a draft; `receive` hears the worker's messages. */
export type Runner = (
  draft: Draft,
  options: RunOptions,
  receive: (message: WorkerMessage) => void,
) => Promise<ValidationSession>;

/** What a workspace is built with. */
export interface WorkspaceDeps {
  /** Where drafts are kept. */
  store: Store;
  /** Where runs are kept. */
  runStore?: Store;
  /** Publishes a snapshot to the renderer. */
  notify: (state: WorkspaceState) => void;
  /** Starts a validation. */
  runner: Runner;
}

/** A problem that stops the draft running; one that stops generation names no step. */
interface Problem {
  /** The step with the problem, if one. */
  stepId?: string;
  /** What is wrong, for the person. */
  message: string;
}

/** The draft's module, or the problems that stop it. */
interface Built {
  /** The module and where each step starts in it (empty when it cannot generate). */
  artifact: Pick<Generated, 'code' | 'mapping'>;
  /** What stops it running. */
  problems: Problem[];
}

/** Everything the renderer shows, in one object. */
export type WorkspaceState = ReturnType<typeof snapshotOf>;

/** Commands that act on the session rather than editing the draft. */
const SESSION: Record<string, (ws: Workspace, command: EditCommand) => WorkspaceState> = {
  undo: (ws) => ws.travel(ws.history, ws.future),
  redo: (ws) => ws.travel(ws.future, ws.history),
  'open-run': (ws, command) => {
    ws.run = ws.runStore!.load(command.id!).run as Run;
    return ws.publish();
  },
  new: (ws) => ws.leaveEmpty().reset(normalizeDraft()).persist(),
  // A file's workflow as a new draft: its own identity, never published, never mid-recording.
  import: (ws, command) => {
    const { name, description, steps, variables, secrets } = command.draft || {};
    return ws.leaveEmpty().reset(normalizeDraft({ name, description, steps, variables, secrets })).persist();
  },
  open: (ws, command) => ws.reset(ws.store.load(command.id!)).publish(),
};

/** The diagnostics bundle for a run: without its draft, code or repair details, redacted. */
function supportBundle(current: Run | null): unknown {
  const run = current && {
    ...current,
    ...{ draft: undefined, code: undefined },
    repairs: current.repairs.map((r) => ({ stepId: r.stepId, draftId: r.draftId })),
  };
  return redact({ schemaVersion: WORKSPACE.SUPPORT_SCHEMA, run });
}

/** The draft's generated module, or the problems that stop it being generated. */
function buildArtifact(draft: Draft): Built {
  let artifact: Built['artifact'] = { code: '', mapping: {} };
  const problems: Problem[] = issues(draft);
  try {
    if (!problems.length) artifact = generate(draft);
  } catch (e) {
    problems.push({ message: (e as Error).message });
  }
  return { artifact, problems };
}

/** Everything the renderer shows about `ws`, in one object. */
function snapshotOf(ws: Workspace) {
  const { artifact, problems } = buildArtifact(ws.draft);
  const head = { draft: ws.draft, ...artifact, issues: problems, library: ws.store.list() };
  const undo = { storageError: ws.storageError, canUndo: !!ws.history.length, canRedo: !!ws.future.length };
  const saved = !!ws.draft.publishedAt && ws.draft.publishedRevision === ws.draft.revision;
  const runHistory = (ws.runStore?.list() || []).filter((item) => !item.error);
  return { ...head, saved, ...undo, runHistory, run: ws.run, support: ws.support() };
}

/** Saves a run record; answers the storage failure, or null. */
function saveRunRecord(runStore: Store, draft: Draft, run: Run): string | null {
  const record = { id: run.id, name: draft.name + ' · ' + run.status, run };
  try {
    runStore.save({ ...record, updatedAt: Date.now() });
    return null;
  } catch (e) {
    return (e as Error).message;
  }
}

/** Refuses to validate while busy or recording, or with nothing to run; throws if the draft cannot generate. */
function checkStartable(ws: Workspace): void {
  if (ws.busy()) throw new Error('A validation is already running');
  if (ws.draft.phase === 'recording') throw new Error('Pause recording before validation');
  if (!ws.draft.steps.some((s) => s.enabled)) throw new Error('Add a step before validation');
  generate(ws.draft);
}

/** One person's workflow workspace. */
export class Workspace {
  /** Where drafts are kept. */
  readonly store: Store;
  /** Where runs are kept. */
  readonly runStore: Store | undefined;
  /** Publishes a snapshot to the renderer. */
  private readonly notify: WorkspaceDeps['notify'];
  /** Starts a validation. */
  private readonly runner: Runner;
  /** The draft being edited. */
  draft: Draft = normalizeDraft();
  /** Earlier versions, for undo. */
  history: Draft[] = [];
  /** Undone versions, for redo. */
  future: Draft[] = [];
  /** The run shown: the current or latest validation. */
  run: Run | null = null;
  /** The last storage failure, shown to the person. */
  storageError: string | null = null;
  /** The running validation's controls, once started. */
  session: ValidationSession | null = null;
  /** A control that arrived before the runner started. */
  private pendingControl: string | null = null;
  /** The throttled publish. */
  private notifyTimer: ReturnType<typeof setTimeout> | null = null;
  /** The throttled run save. */
  private runSaveTimer: ReturnType<typeof setTimeout> | null = null;

  /** `store` holds drafts, `runStore` runs; `notify` publishes snapshots; `runner` starts a validation. */
  constructor({ store, runStore, notify, runner }: WorkspaceDeps) {
    this.store = store;
    this.runStore = runStore;
    this.notify = notify;
    this.runner = runner;
    const recent = store.list().find((d) => !d.error);
    if (recent) this.draft = store.load(recent.id);
    this.run = recoverRuns(runStore, this.draft.id);
    pruneRuns(this.runStore);
  }

  /** The diagnostics bundle: the current run without its draft, code or repair details, redacted. */
  support(): unknown {
    return supportBundle(this.run);
  }

  /** Everything the renderer shows, in one object. */
  snapshot(): WorkspaceState {
    return snapshotOf(this);
  }

  /** Sends a snapshot to the renderer and returns it. */
  publish(): WorkspaceState {
    const state = this.snapshot();
    this.notify(state);
    return state;
  }

  /** Saves the draft (a failure is shown, not thrown) and publishes. */
  persist(): WorkspaceState {
    this.draft.updatedAt = Date.now();
    try {
      this.store.save(this.draft);
      this.storageError = null;
    } catch (e) {
      this.storageError = (e as Error).message;
    }
    return this.publish();
  }

  /** Saves the current run record; a failure is shown, not thrown. */
  saveRun(): void {
    if (this.runStore && this.run)
      this.storageError = saveRunRecord(this.runStore, this.draft, this.run) ?? this.storageError;
  }

  /** Whether a validation is going. */
  busy(): boolean {
    return !!this.run && isActive(this.run);
  }

  /** Forgets the current draft when it has no steps, so leaving it never strands an empty entry in the library. */
  leaveEmpty(): this {
    if (!this.draft.steps.length) this.store.remove(this.draft.id);
    return this;
  }

  /** Makes `draft` current with a clean history and no run shown. */
  reset(draft: Draft): this {
    Object.assign(this, { draft, history: [], future: [], run: null });
    return this;
  }

  /** Undo or redo: moves the draft from one history stack to the other. */
  travel(from: Draft[], to: Draft[]): WorkspaceState {
    if (from.length) {
      to.push(this.draft);
      this.draft = from.pop()!;
    }
    return this.persist();
  }

  /** Refuses edits while validating or recording. */
  private assertEditable(): void {
    if (this.busy()) throw new Error('Stop validation before editing its draft.');
    if (this.draft.phase === 'recording') throw new Error('Pause recording before editing steps.');
  }

  /** Applies an editor command; refused while validating or recording. */
  edit(command: EditCommand): WorkspaceState {
    this.assertEditable();
    if (Object.hasOwn(SESSION, command.type)) return SESSION[command.type](this, command);
    const before = structuredClone(this.draft);
    const draft = structuredClone(this.draft);
    applyEdit(draft, command);
    this.draft = normalizeDraft({ ...draft, revision: draft.revision + 1 });
    return this.remember(before);
  }

  /** Records `before` for undo (bounded), clears redo, and saves. */
  private remember(before: Draft): WorkspaceState {
    this.history.push(before);
    if (this.history.length > WORKSPACE.MAX_HISTORY) this.history.shift();
    this.future = [];
    return this.persist();
  }

  /** Takes the recorder's latest steps and secrets into the draft. */
  capture(steps: unknown[], secrets: Iterable<string>, recording: boolean): WorkspaceState {
    const changes = { steps, secrets: [...secrets], phase: recording ? 'recording' : 'paused' };
    this.draft = normalizeDraft({ ...this.draft, ...changes, revision: this.draft.revision + 1 });
    return this.persist();
  }

  /** Starts validating the draft; a runner failure marks the run failed. */
  async start(options: RunOptions): Promise<WorkspaceState> {
    checkStartable(this);
    Object.assign(this, { session: null, pendingControl: null, run: newRun(this.draft) });
    this.saveRun();
    pruneRuns(this.runStore);
    this.publish();
    await this.launch(options).catch((e: Error) => this.failRun(e));
    return this.publish();
  }

  /** The runner could not start: mark the run failed and save it. */
  private failRun(e: Error): void {
    Object.assign(this.run!, { status: 'failed', error: redact(e.message) });
    this.saveRun();
  }

  /** Starts the runner, then passes on any control that arrived while it started. */
  private async launch(options: RunOptions): Promise<void> {
    const run = this.run;
    this.session = await this.runner(structuredClone(this.draft), options, (message) => this.receive(message, run));
    if (this.pendingControl) {
      this.session.control(this.pendingControl);
      this.pendingControl = null;
    }
    if (this.run!.status === 'starting') this.run!.status = 'running';
  }

  /** A worker message for `run`: fold it in, then publish (at once when finished, else throttled). An earlier run's is ignored. */
  receive(message: WorkerMessage, run: Run | null = this.run): void {
    if (!this.run || run !== this.run) return;
    applyMessage(this as Workspace & WithRun, message);
    if (message.type === 'finished') this.finish();
    else this.schedule();
  }

  /** The run finished: cancel pending updates, save and publish now. */
  private finish(): void {
    clearTimeout(this.notifyTimer ?? undefined);
    clearTimeout(this.runSaveTimer ?? undefined);
    this.notifyTimer = this.runSaveTimer = null;
    this.saveRun();
    this.publish();
  }

  /** Publishes and saves progress at most once per interval each. */
  private schedule(): void {
    const later = (fn: () => void, ms: number): ReturnType<typeof setTimeout> => setTimeout(fn, ms);
    if (!this.notifyTimer)
      this.notifyTimer = later(() => ((this.notifyTimer = null), this.publish()), WORKSPACE.NOTIFY_MS);
    if (!this.runSaveTimer) {
      this.runSaveTimer = later(() => ((this.runSaveTimer = null), this.saveRun()), WORKSPACE.SAVE_RUN_MS);
    }
  }

  /** Pause, resume, step or stop the running validation (queued until the runner has started). */
  control(command: string): WorkspaceState {
    if (!this.busy()) throw new Error('No active validation');
    if (!['pause', 'resume', 'step', 'stop'].includes(command)) throw new Error('Unknown run control');
    if (this.session) this.session.control(command);
    else this.pendingControl = command;
    if (command === 'stop') this.run!.status = 'stopping';
    return this.publish();
  }
}
