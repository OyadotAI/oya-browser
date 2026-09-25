/**
 * Unit tests for the Routines pane's IPC: each channel reaches the scheduler
 * with its arguments.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { ROUTINE_HANDLERS } = require('../../../../main/ipc/routines.cjs');

/** A scheduler that records what it was asked, answering each with the method's name. */
function fakeRoutines() {
  const calls = [];
  const record =
    (name) =>
    (...args) => (calls.push([name, ...args]), name);
  const methods = ['refresh', 'save', 'remove', 'setEnabled', 'clearHistory', 'stop', 'runNow'];
  return { calls, ...Object.fromEntries(methods.map((m) => [m, record(m)])) };
}

describe('ROUTINE_HANDLERS', () => {
  it('sends every channel to the scheduler', () => {
    const ctx = { routines: fakeRoutines() };
    const call = (channel, ...args) => ROUTINE_HANDLERS[channel](ctx, null, ...args);
    assert.equal(call('list-routines'), 'refresh');
    call('save-routine', { name: 'A' });
    call('delete-routine', 'r1');
    call('set-routine-enabled', 'r1', false);
    call('clear-routine-history', 'r1');
    call('stop-routine', 'r1');
    call('run-routine-now', 'r1');
    assert.deepEqual(ctx.routines.calls.slice(1), [
      ['save', { name: 'A' }],
      ['remove', 'r1'],
      ['setEnabled', 'r1', false],
      ['clearHistory', 'r1'],
      ['stop', 'r1'],
      ['runNow', 'r1'],
    ]);
  });
});
