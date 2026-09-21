/**
 * The workflow workspace: the draft being edited, its undo history, the local
 * draft library, and validation runs. The main process drives it; every change
 * is saved and published to the renderer as one snapshot.
 *
 * Behind it, in workspace/: edits (the editor command map) and runs (run
 * records, recovery, pruning and worker messages).
 */
const { normalizeDraft, generate, issues } = require('./workflow.cjs');
const { redact } = require('./diagnostics.cjs');
const { WORKSPACE } = require('./constants.cjs');
const { applyEdit } = require('./workspace/edits.cjs');
const { isActive, newRun, recoverRuns, pruneRuns, applyMessage } = require('./workspace/runs.cjs');

/** Commands that act on the session rather than editing the draft. */
const SESSION = {
  undo: (ws) => ws.travel(ws.history, ws.future),
  redo: (ws) => ws.travel(ws.future, ws.history),
  'open-run': (ws, command) => {
    ws.run = ws.runStore.load(command.id).run;
    return ws.publish();
  },
  new: (ws) => ws.leaveEmpty().reset(normalizeDraft()).persist(),
  open: (ws, command) => ws.reset(ws.store.load(command.id)).publish(),
};

/** The diagnostics bundle for a run: without its draft, code or repair details, redacted. */
function supportBundle(current) {
  const run = current && {
    ...current,
    ...{ draft: undefined, code: undefined },
    repairs: current.repairs.map((r) => ({ stepId: r.stepId, draftId: r.draftId })),
  };
  return redact({ schemaVersion: WORKSPACE.SUPPORT_SCHEMA, run });
}

/** The draft's generated module, or the problems that stop it being generated. */
function buildArtifact(draft) {
  let artifact = { code: '', mapping: {} };
  const problems = issues(draft);
  try {
    if (!problems.length) artifact = generate(draft);
  } catch (e) {
    problems.push({ message: e.message });
  }
  return { artifact, problems };
}

/** One person's workflow workspace. */
class Workspace {
  /** `store` holds drafts, `runStore` runs; `notify` publishes snapshots; `runner` starts a validation. */
  constructor({ store, runStore, notify, runner }) {
    Object.assign(this, { store, runStore, notify, runner });
    Object.assign(this, { draft: normalizeDraft(), history: [], future: [], run: null, storageError: null });
    const recent = store.list().find((d) => !d.error);
    if (recent) this.draft = store.load(recent.id);
    this.run = recoverRuns(runStore, this.draft.id);
    this.pruneRuns();
  }

  /** The diagnostics bundle: the current run without its draft, code or repair details, redacted. */
  support() {
    return supportBundle(this.run);
  }

  /** Everything the renderer shows, in one object. */
  snapshot() {
    const { artifact, problems } = buildArtifact(this.draft);
    const head = { draft: this.draft, ...artifact, issues: problems, library: this.store.list() };
    const undo = { storageError: this.storageError, canUndo: !!this.history.length, canRedo: !!this.future.length };
    head.saved = !!this.draft.publishedAt && this.draft.publishedRevision === this.draft.revision;
    const runHistory = (this.runStore?.list() || []).filter((item) => !item.error);
    return { ...head, ...undo, runHistory, run: this.run, support: this.support() };
  }

  /** Sends a snapshot to the renderer and returns it. */
  publish() {
    const state = this.snapshot();
    this.notify(state);
    return state;
  }

  /** Saves the draft (a failure is shown, not thrown) and publishes. */
  persist() {
    this.draft.updatedAt = Date.now();
    try {
      this.store.save(this.draft);
      this.storageError = null;
    } catch (e) {
      this.storageError = e.message;
    }
    return this.publish();
  }

  /** Deletes stored runs past the retention limits. */
  pruneRuns() {
    pruneRuns(this.runStore);
  }

  /** Saves the current run record; a failure is shown, not thrown. */
  saveRun() {
    if (!this.runStore || !this.run) return;
    const record = { id: this.run.id, name: this.draft.name + ' · ' + this.run.status, run: this.run };
    try {
      this.runStore.save({ ...record, updatedAt: Date.now() });
    } catch (e) {
      this.storageError = e.message;
    }
  }

  /** Whether a validation is going. */
  busy() {
    return this.run && isActive(this.run);
  }

  /** Switches to `draft` with a clean history and no run shown. */
  /** Forgets the current draft when it has no steps, so leaving it never strands an empty entry in the library. */
  leaveEmpty() {
    if (!this.draft.steps.length) this.store.remove(this.draft.id);
    return this;
  }

  /** Makes `draft` current with a clean history. */
  reset(draft) {
    Object.assign(this, { draft, history: [], future: [], run: null });
    return this;
  }

  /** Undo or redo: moves the draft from one history stack to the other. */
  travel(from, to) {
    if (from.length) {
      to.push(this.draft);
      this.draft = from.pop();
    }
    return this.persist();
  }

  /** Refuses edits while validating or recording. */
  assertEditable() {
    if (this.busy()) throw new Error('Stop validation before editing its draft.');
    if (this.draft.phase === 'recording') throw new Error('Pause recording before editing steps.');
  }

  /** Applies an editor command; refused while validating or recording. */
  edit(command) {
    this.assertEditable();
    if (Object.hasOwn(SESSION, command.type)) return SESSION[command.type](this, command);
    const before = structuredClone(this.draft);
    const draft = structuredClone(this.draft);
    applyEdit(draft, command);
    this.draft = normalizeDraft({ ...draft, revision: draft.revision + 1 });
    return this.remember(before);
  }

  /** Records `before` for undo (bounded), clears redo, and saves. */
  remember(before) {
    this.history.push(before);
    if (this.history.length > WORKSPACE.MAX_HISTORY) this.history.shift();
    this.future = [];
    return this.persist();
  }

  /** Takes the recorder's latest steps and secrets into the draft. */
  capture(steps, secrets, recording) {
    const changes = { steps, secrets: [...secrets], phase: recording ? 'recording' : 'paused' };
    this.draft = normalizeDraft({ ...this.draft, ...changes, revision: this.draft.revision + 1 });
    return this.persist();
  }

  /** Refuses to validate while busy or recording, or with nothing to run; throws if the draft cannot generate. */
  checkStartable() {
    if (this.busy()) throw new Error('A validation is already running');
    if (this.draft.phase === 'recording') throw new Error('Pause recording before validation');
    if (!this.draft.steps.some((s) => s.enabled)) throw new Error('Add a step before validation');
    generate(this.draft);
  }

  /** Starts validating the draft; a runner failure marks the run failed. */
  async start(options) {
    this.checkStartable();
    Object.assign(this, { session: null, pendingControl: null, run: newRun(this.draft) });
    this.saveRun();
    this.pruneRuns();
    this.publish();
    await this.launch(options).catch((e) => this.failRun(e));
    return this.publish();
  }

  /** The runner could not start: mark the run failed and save it. */
  failRun(e) {
    Object.assign(this.run, { status: 'failed', error: redact(e.message) });
    this.saveRun();
  }

  /** Starts the runner, then passes on any control that arrived while it started. */
  async launch(options) {
    const run = this.run;
    this.session = await this.runner(structuredClone(this.draft), options, (message) => this.receive(message, run));
    if (this.pendingControl) {
      this.session.control(this.pendingControl);
      this.pendingControl = null;
    }
    if (this.run.status === 'starting') this.run.status = 'running';
  }

  /** A worker message for `run`: fold it in, then publish (at once when finished, else throttled). An earlier run's is ignored. */
  receive(message, run = this.run) {
    if (!this.run || run !== this.run) return;
    applyMessage(this, message);
    if (message.type === 'finished') this.finish();
    else this.schedule();
  }

  /** The run finished: cancel pending updates, save and publish now. */
  finish() {
    clearTimeout(this.notifyTimer);
    clearTimeout(this.runSaveTimer);
    this.notifyTimer = this.runSaveTimer = null;
    this.saveRun();
    this.publish();
  }

  /** Publishes and saves progress at most once per interval each. */
  schedule() {
    const later = (fn, ms) => setTimeout(fn, ms);
    if (!this.notifyTimer)
      this.notifyTimer = later(() => ((this.notifyTimer = null), this.publish()), WORKSPACE.NOTIFY_MS);
    if (!this.runSaveTimer) {
      this.runSaveTimer = later(() => ((this.runSaveTimer = null), this.saveRun()), WORKSPACE.SAVE_RUN_MS);
    }
  }

  /** Pause, resume, step or stop the running validation (queued until the runner has started). */
  control(command) {
    if (!this.busy()) throw new Error('No active validation');
    if (!['pause', 'resume', 'step', 'stop'].includes(command)) throw new Error('Unknown run control');
    if (this.session) this.session.control(command);
    else this.pendingControl = command;
    if (command === 'stop') this.run.status = 'stopping';
    return this.publish();
  }
}

module.exports = { Workspace };
