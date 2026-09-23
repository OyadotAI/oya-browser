/**
 * Unit tests for the Routines pane's IPC: each channel reaches the scheduler,
 * and "Run now" answers at once instead of holding the pane for the whole run.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { ROUTINE_HANDLERS } = require('../../../../main/ipc/routines.cjs');

/** A scheduler that records what it was asked. */
function fakeRoutines() {
  const calls = [];
  const snapshot = { routines: [], running: null };
  return {
    calls,
    snapshot: () => snapshot,
    save: (routine) => (calls.push(['save', routine]), snapshot),
    remove: (id) => (calls.push(['remove', id]), snapshot),
    runNow: (id) => (calls.push(['run', id]), new Promise(() => {})),
  };
}

describe('ROUTINE_HANDLERS', () => {
  it('lists, saves and deletes through the scheduler', () => {
    const ctx = { routines: fakeRoutines() };
    assert.deepEqual(ROUTINE_HANDLERS['list-routines'](ctx), { routines: [], running: null });
    ROUTINE_HANDLERS['save-routine'](ctx, null, { name: 'A' });
    ROUTINE_HANDLERS['delete-routine'](ctx, null, 'r1');
    assert.deepEqual(ctx.routines.calls, [
      ['save', { name: 'A' }],
      ['remove', 'r1'],
    ]);
  });

  it('answers "Run now" at once, while the run goes on', () => {
    const ctx = { routines: fakeRoutines() };
    assert.deepEqual(ROUTINE_HANDLERS['run-routine-now'](ctx, null, 'r1'), { routines: [], running: null });
    assert.deepEqual(ctx.routines.calls, [['run', 'r1']]);
  });
});
