/**
 * Unit tests for scripts/workspace/runs.cjs: run records, crash recovery,
 * retention, and folding worker messages into a run.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const runs = require('../../../../scripts/workspace/runs.cjs');
const { normalizeDraft } = require('../../../../scripts/workflow.cjs');
const { MemoryStore } = require('../../support/stores.cjs');

const DAY = 86400000;
const draft = normalizeDraft({ id: 'd', name: 'Login', steps: [{ id: 'a', action: 'press_key', key: 'A' }] });

describe('workspace runs', () => {
  it('starts a run with a copy of the draft and its exact code', () => {
    const run = runs.newRun(draft);
    assert.deepEqual([run.draftId, run.status, run.events, run.repairs], ['d', 'starting', [], []]);
    assert.match(run.code, /keyboard\.press/);
    assert.notEqual(run.draft, draft);
  });

  it('knows which statuses are still going', () => {
    assert.ok(['starting', 'running', 'paused', 'stopping'].every((status) => runs.isActive({ status })));
    assert.ok(!runs.isActive({ status: 'failed' }) && !runs.isActive(null));
  });

  it('interrupts runs a crash left going, and finds the draft’s latest run', () => {
    const store = new MemoryStore({
      r1: { id: 'r1', updatedAt: 3, run: { draftId: 'd', status: 'running' } },
      r2: { id: 'r2', updatedAt: 2, run: { draftId: 'd', status: 'succeeded' } },
    });
    const latest = runs.recoverRuns(store, 'd');
    assert.equal(latest.status, 'interrupted');
    assert.match(store.load('r1').run.error, /Oya closed during validation/);
    assert.equal(runs.recoverRuns(null, 'd'), null);
  });

  it('prunes runs past the count, the age or the total size', () => {
    const now = Date.now();
    const records = Object.fromEntries(
      Array.from({ length: 32 }, (_, i) => [`r${i}`, { id: `r${i}`, updatedAt: now - i }]),
    );
    records.old = { id: 'old', updatedAt: now - 8 * DAY };
    const store = new MemoryStore(records);
    runs.pruneRuns(store);
    assert.deepEqual(store.removed.sort(), ['old', 'r30', 'r31']);
  });

  it('keeps a bounded event log and follows pause and resume', () => {
    const workspace = { run: { status: 'running', events: Array(2000).fill({}), repairs: [] } };
    runs.applyMessage(workspace, { type: 'event', event: { status: 'paused' } });
    assert.deepEqual([workspace.run.events.length, workspace.run.status], [2000, 'paused']);
    runs.applyMessage(workspace, { type: 'event', event: { status: 'running' } });
    assert.equal(workspace.run.status, 'running');
  });

  it('saves a repair as a new draft and lists it on the run', () => {
    const store = new MemoryStore();
    const workspace = { draft, store, run: { events: [], repairs: [] } };
    runs.applyMessage(workspace, { type: 'repair', stepId: 'a', original: 1, replacement: 2, draft });
    const [repair] = workspace.run.repairs;
    const saved = store.load(repair.draftId);
    assert.deepEqual([saved.name, saved.repairedFrom, repair.stepId], ['Login-repair', 'd', 'a']);
  });

  it('notes a repair it could not save', () => {
    const workspace = { draft, store: new MemoryStore({}, { failSave: true }), run: { events: [], repairs: [] } };
    runs.applyMessage(workspace, { type: 'repair', stepId: 'a', draft });
    assert.match(workspace.storageError, /unavailable/);
    assert.equal(workspace.run.events[0].kind, 'attention');
  });

  it('records the finish, and ignores unknown messages', () => {
    const workspace = { run: { status: 'running' } };
    runs.applyMessage(workspace, { type: 'mystery' });
    assert.equal(workspace.run.status, 'running');
    runs.applyMessage(workspace, { type: 'finished', status: 'failed' });
    assert.equal(workspace.run.status, 'failed');
    assert.ok(workspace.run.finishedAt);
  });
});
