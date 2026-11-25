/**
 * ResumableRuns: a chat run keeps going for a grace window after its listener
 * leaves, its answer can be collected by the caller that started it, and Stop
 * or the window's end stops it.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ResumableRuns, runKey } from '../../../../src/modules/agent/resumable-runs.ts';

/** The grace window the tests run with. */
const GRACE = 1000;

/** A run that ends when the test says, and the signal it was given. */
function pending() {
  const out: { signal?: AbortSignal; resolve?: (v: unknown) => void; reject?: (e: Error) => void } = {};
  const work = (signal: AbortSignal) =>
    new Promise((resolve, reject) => Object.assign(out, { signal, resolve, reject }));
  return { out, work };
}

/** Lets promise callbacks run. */
const tick = () => new Promise((resolve) => setImmediate(resolve));

describe('ResumableRuns', () => {
  beforeEach(() => mock.timers.enable({ apis: ['setTimeout'] }));
  afterEach(() => mock.timers.reset());

  it('keeps a run going when its listener leaves, and stops it when the grace window ends', () => {
    const runs = new ResumableRuns(GRACE);
    const { out, work } = pending();
    runs.release(runs.start('k', 'key', 't', work));
    mock.timers.tick(GRACE - 1);
    assert.equal(out.signal.aborted, false);
    mock.timers.tick(1);
    assert.equal(out.signal.aborted, true);
  });

  it('a caller that comes back stops the countdown', () => {
    const runs = new ResumableRuns(GRACE);
    const { out, work } = pending();
    runs.release(runs.start('k', 'key', 't', work));
    runs.attach('k', 'key', 't');
    mock.timers.tick(GRACE * 2);
    assert.equal(out.signal.aborted, false);
  });

  it('the first response closing late does not start the countdown while another listens', () => {
    const runs = new ResumableRuns(GRACE);
    const { out, work } = pending();
    const first = runs.start('k', 'key', 't', work);
    runs.attach('k', 'key', 't');
    runs.release(first);
    mock.timers.tick(GRACE * 2);
    assert.equal(out.signal.aborted, false);
  });

  it('hands a finished answer to a caller that comes back within the window, and forgets it after', async () => {
    const runs = new ResumableRuns(GRACE);
    const { out, work } = pending();
    runs.release(runs.start('k', 'key', 't', work));
    out.resolve({ text: 'done' });
    await tick();
    assert.deepEqual(await runs.attach('k', 'key', 't').answer, { text: 'done' });
    mock.timers.tick(GRACE);
    assert.throws(() => runs.attach('k', 'key', 't'), { status: 404, code: 'chat_run_gone' });
  });

  it("refuses another key, or another run's token, as if there were no run", () => {
    const runs = new ResumableRuns(GRACE);
    runs.start('k', 'key', 't', pending().work);
    assert.throws(() => runs.attach('k', 'other', 't'), { status: 404, code: 'chat_run_gone' });
    assert.throws(() => runs.attach('k', 'key', 'stale'), { status: 404, code: 'chat_run_gone' });
    assert.throws(() => runs.attach('nope', 'key', 't'), { status: 404, code: 'chat_run_gone' });
  });

  it('a new run on the key stops the one before it, whose end does not drop the new one', async () => {
    const runs = new ResumableRuns(GRACE);
    const old = pending();
    runs.start('k', 'key', 'a', old.work);
    runs.start('k', 'key', 'b', pending().work);
    assert.equal(old.out.signal.aborted, true);
    old.out.resolve('late');
    await tick();
    mock.timers.tick(GRACE);
    assert.ok(runs.attach('k', 'key', 'b'));
  });

  it('a run that fails rejects for whoever collects it, with no unhandled rejection', async () => {
    const runs = new ResumableRuns(GRACE);
    const { out, work } = pending();
    runs.release(runs.start('k', 'key', 't', work));
    out.reject(new Error('model down'));
    await tick();
    await assert.rejects(runs.attach('k', 'key', 't').answer, /model down/);
  });

  it('a run that throws before it starts is a rejected answer, not a throw', async () => {
    const runs = new ResumableRuns(GRACE);
    const run = runs.start('k', 'key', 't', () => {
      throw new Error('bad start');
    });
    await assert.rejects(run.answer, /bad start/);
  });

  it("stop ends the owner's unfinished run at once, and nobody else's", async () => {
    const runs = new ResumableRuns(GRACE);
    const { out, work } = pending();
    runs.start('k', 'key', 't', work);
    assert.equal(runs.stop('k', 'other', 't'), false);
    assert.equal(out.signal.aborted, false);
    assert.equal(runs.stop('k', 'key', 't'), true);
    assert.equal(out.signal.aborted, true);
    out.resolve('stopped');
    await tick();
    assert.equal(runs.stop('k', 'key', 't'), false);
    assert.equal(runs.stop('none', 'key', 't'), false);
  });

  it('a stop that names an earlier run does not end the run started after it', () => {
    const runs = new ResumableRuns(GRACE);
    runs.start('k', 'key', 'first', pending().work);
    const next = pending();
    runs.start('k', 'key', 'second', next.work);
    assert.equal(runs.stop('k', 'key', 'first'), false);
    assert.equal(next.out.signal.aborted, false);
  });

  it("refuses to replace another key's unfinished run, which keeps going", () => {
    const runs = new ResumableRuns(GRACE);
    const { out, work } = pending();
    runs.start('k', 'key', 't', work);
    assert.throws(() => runs.start('k', 'other', 't2', pending().work), { status: 409 });
    assert.equal(out.signal.aborted, false);
  });

  it('keys a run by browser and lane', () => {
    assert.equal(runKey('oya-1', 'tab-3'), 'oya-1#tab-3');
  });
});
