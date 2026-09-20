/**
 * Unit tests for scripts/workspace.cjs: editing with undo and redo, the
 * snapshot the renderer shows, and a validation run from start to finish.
 */
const { describe, it, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { Workspace } = require('../../../scripts/workspace.cjs');
const { MemoryStore } = require('../support/stores.cjs');

const STEPS = [
  { id: 'a', action: 'navigate', url: 'https://x.test' },
  { id: 'b', action: 'click', candidates: [{ kind: 'css', value: '#go' }] },
];

/** A workspace over memory stores; `runner` answers validation starts. */
function workspaceWith({ drafts = {}, runs = {}, runner, failSave = false } = {}) {
  const published = [];
  const controls = [];
  const session = { control: (c) => controls.push(c) };
  const ws = new Workspace({
    store: new MemoryStore(drafts, { failSave }),
    runStore: new MemoryStore(runs),
    notify: (state) => published.push(state),
    runner: runner || (async (draft, options, receive) => ((ws.receiveFromWorker = receive), session)),
  });
  return { ws, published, controls };
}

describe('Workspace', () => {
  afterEach(() => mock.timers.reset());

  it('opens the most recent readable draft', () => {
    const { ws } = workspaceWith({ drafts: { d: { id: 'd', name: 'Mine', updatedAt: 5, steps: STEPS } } });
    assert.equal(ws.draft.name, 'Mine');
    assert.equal(workspaceWith().ws.draft.name, 'Untitled workflow');
  });

  it('edits, undoes and redoes, bumping the revision and saving each time', () => {
    const { ws, published } = workspaceWith();
    ws.capture(STEPS, new Set(), false);
    ws.edit({ type: 'update', id: 'b', patch: { timeout: 2500 } });
    assert.deepEqual([ws.draft.steps[1].timeout, ws.draft.revision], [2500, 2]);
    ws.edit({ type: 'undo' });
    assert.equal(ws.draft.steps[1].timeout, 15000);
    ws.edit({ type: 'redo' });
    assert.equal(ws.draft.steps[1].timeout, 2500);
    assert.equal(published.at(-1).canUndo, true);
    assert.equal(ws.store.load(ws.draft.id).steps[1].timeout, 2500);
  });

  it('keeps at most a hundred undo steps', () => {
    const { ws } = workspaceWith();
    for (let i = 0; i < 105; i++) ws.edit({ type: 'metadata', name: 'n' + i });
    assert.equal(ws.history.length, 100);
  });

  it('clears the publish mark when the draft changes', () => {
    const { ws } = workspaceWith();
    ws.draft.publishedAt = 1;
    ws.edit({ type: 'metadata', name: 'x' });
    assert.equal(ws.draft.publishedAt, undefined);
  });

  it('starts a new draft or opens a saved one with a clean history', () => {
    const { ws } = workspaceWith();
    ws.store.save({ id: 'saved', name: 'Saved', updatedAt: 1, steps: [] });
    ws.edit({ type: 'metadata', name: 'x' });
    ws.edit({ type: 'new' });
    assert.deepEqual([ws.draft.name, ws.history.length], ['Untitled workflow', 0]);
    ws.edit({ type: 'open', id: 'saved' });
    assert.equal(ws.draft.name, 'Saved');
  });

  it('refuses edits while recording', () => {
    const { ws } = workspaceWith();
    ws.capture(STEPS, [], true);
    assert.throws(() => ws.edit({ type: 'undo' }), /Pause recording/);
  });

  it('shows a storage failure instead of throwing', () => {
    const { ws, published } = workspaceWith({ failSave: true });
    ws.edit({ type: 'metadata', name: 'x' });
    assert.match(published.at(-1).storageError, /unavailable/);
  });

  it('snapshots the generated code, or the issues that block it', () => {
    const { ws } = workspaceWith();
    ws.capture(STEPS, [], false);
    assert.match(ws.snapshot().code, /p\.goto/);
    ws.capture([{ id: 'x', action: 'click' }], [], false);
    const snap = ws.snapshot();
    assert.equal(snap.code, '');
    assert.equal(snap.issues[0].message, 'Pick a target before validation.');
  });

  it('refuses to validate while recording or with nothing to run', async () => {
    const { ws } = workspaceWith();
    await assert.rejects(ws.start({}), /Add a step/);
    ws.capture(STEPS, [], true);
    await assert.rejects(ws.start({}), /Pause recording/);
  });

  it('runs a validation: running, controls, progress, then finished', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    const { ws, published, controls } = workspaceWith();
    ws.capture(STEPS, [], false);
    await ws.start({});
    assert.equal(ws.run.status, 'running');
    assert.ok(ws.busy());
    assert.throws(() => ws.edit({ type: 'undo' }), /Stop validation/);
    await assert.rejects(ws.start({}), /already running/);
    ws.control('pause');
    assert.deepEqual(controls, ['pause']);
    assert.throws(() => ws.control('jump'), /Unknown run control/);
    const before = published.length;
    ws.receiveFromWorker({ type: 'event', event: { kind: 'step', status: 'paused' } });
    ws.receiveFromWorker({ type: 'event', event: { kind: 'step', status: 'running' } });
    assert.equal(published.length, before, 'progress is throttled');
    mock.timers.tick(100);
    assert.equal(published.length, before + 1);
    ws.receiveFromWorker({ type: 'finished', status: 'succeeded', assertions: 0 });
    assert.equal(ws.run.status, 'succeeded');
    assert.ok(!ws.busy());
    assert.equal(ws.runStore.load(ws.run.id).run.status, 'succeeded');
    assert.throws(() => ws.control('stop'), /No active validation/);
  });

  it('queues a control sent before the runner has started', async () => {
    let release;
    const gate = new Promise((resolve) => (release = resolve));
    const controls = [];
    const { ws } = workspaceWith({ runner: async () => (await gate, { control: (c) => controls.push(c) }) });
    ws.capture(STEPS, [], false);
    const started = ws.start({});
    ws.control('stop');
    assert.equal(ws.run.status, 'stopping');
    release();
    await started;
    assert.deepEqual(controls, ['stop']);
  });

  it('marks the run failed, redacted, when the runner cannot start', async () => {
    const { ws } = workspaceWith({
      runner: async () => {
        throw new Error('no port at https://u:p@x.test/?k=1');
      },
    });
    ws.capture(STEPS, [], false);
    await ws.start({});
    assert.deepEqual([ws.run.status, ws.run.error], ['failed', 'no port at https://x.test/']);
  });

  it('recovers an interrupted run for the open draft', () => {
    const drafts = { d: { id: 'd', updatedAt: 1, steps: STEPS } };
    const runs = {
      r: { id: 'r', updatedAt: Date.now(), run: { id: 'r', draftId: 'd', status: 'paused', repairs: [] } },
    };
    const { ws } = workspaceWith({ drafts, runs });
    assert.equal(ws.run.status, 'interrupted');
  });

  it('builds a redacted support bundle without the draft or code', async () => {
    const { ws } = workspaceWith();
    ws.capture(STEPS, [], false);
    await ws.start({});
    ws.run.repairs.push({ stepId: 'b', draftId: 'r', original: { value: 'secret' } });
    const bundle = ws.support();
    assert.equal(bundle.schemaVersion, 1);
    assert.equal(bundle.run.draft, undefined);
    assert.equal(bundle.run.code, undefined);
    assert.deepEqual(bundle.run.repairs, [{ stepId: 'b', draftId: 'r' }]);
  });
});
