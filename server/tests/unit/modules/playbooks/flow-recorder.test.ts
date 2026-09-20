/**
 * Unit tests for a demonstrated flow: starting, polling and stopping the
 * browser's recorder, merging API navigations, caps, expiry and discarding.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import * as flows from '../../../../src/modules/playbooks/flow-recorder.ts';
import { MAX_STEPS, RECORD_MAX_MS, RECORD_POLL_MS } from '../../../../src/modules/playbooks/constants.ts';

let n = 0;
let browserId: string;

/** A browser recorder that hands out the queued steps on each poll. */
function recorderStub(queue: any[][] = [], extra: any = {}) {
  const modes: string[] = [];
  const dispatch = async (_action, { mode }) => {
    modes.push(mode);
    if (extra.fail?.(mode)) return { ok: false, error: extra.error };
    return { ok: true, data: { steps: queue.shift() || [], secrets: extra.secrets } };
  };
  return { dispatch, modes };
}

/** Lets queued promise callbacks run. */
const settle = () => new Promise((r) => setImmediate(r));

describe('flow recorder', () => {
  beforeEach(() => {
    browserId = `b-flow-${++n}`;
    mock.timers.enable({ apis: ['setInterval', 'setTimeout', 'Date'], now: 1_700_000_000_000 });
  });
  afterEach(async () => {
    mock.timers.reset();
    await flows.stop(browserId, recorderStub().dispatch).catch(() => {});
    await flows.discard(browserId).catch(() => {});
  });

  it('starts the browser’s recorder and returns what it has so far', async () => {
    const { dispatch, modes } = recorderStub([[{ id: 'a', action: 'click', t: 1 }]]);
    const snap = await flows.start(browserId, dispatch);
    assert.deepEqual(modes, ['start']);
    assert.equal(snap.recording, true);
    assert.deepEqual(snap.steps, [{ id: 'a', action: 'click', t: 1 }]);
    assert.equal(flows.isRecording(browserId), true);
  });

  it('returns the running recording rather than starting another', async () => {
    const { dispatch, modes } = recorderStub();
    await flows.start(browserId, dispatch);
    await flows.start(browserId, dispatch);
    assert.deepEqual(modes, ['start']);
  });

  it('drops a recording whose start the browser refuses, with 422', async () => {
    const { dispatch } = recorderStub([], { fail: () => true, error: 'no recorder' });
    await assert.rejects(flows.start(browserId, dispatch), { status: 422, message: 'no recorder' });
    assert.equal(flows.isRecording(browserId), false);
    assert.deepEqual(await flows.status(browserId), { recording: false, steps: [], secrets: [] });
  });

  it('polls every few seconds, keeping each step once and every secret name', async () => {
    const step = { id: 's1', action: 'type', t: 2 };
    const { dispatch, modes } = recorderStub([[], [step], [step, { action: 'click', t: 3 }]], { secrets: ['pw'] });
    await flows.start(browserId, dispatch);
    mock.timers.tick(RECORD_POLL_MS);
    await settle();
    mock.timers.tick(RECORD_POLL_MS);
    await settle();
    const snap = await flows.status(browserId);
    assert.deepEqual(modes, ['start', 'drain', 'drain']);
    assert.deepEqual(
      snap.steps.map((s) => s.action),
      ['type', 'click'],
    );
    assert.deepEqual(snap.secrets, ['pw']);
  });

  it('polls the browser when asked for status with a dispatcher', async () => {
    const { dispatch, modes } = recorderStub([[], [{ id: 'x', action: 'click' }]]);
    await flows.start(browserId, dispatch);
    const snap = await flows.status(browserId, dispatch);
    assert.deepEqual(modes, ['start', 'drain']);
    assert.equal(snap.steps.length, 1);
  });

  it('merges navigations sent through the API into the recording, in time order', async () => {
    const { dispatch } = recorderStub([[{ id: 'c', action: 'click', t: Date.now() + 10 }]]);
    await flows.start(browserId, dispatch);
    flows.noteCommand(browserId, 'navigate', { url: 'https://a.test/' });
    flows.noteCommand(browserId, 'navigate', { url: 'javascript:void(0)' });
    flows.noteCommand(browserId, 'click', { url: 'https://b.test/' });
    const snap = await flows.status(browserId);
    assert.deepEqual(
      snap.steps.map((s) => s.action),
      ['navigate', 'click'],
    );
  });

  it('ignores an API navigation when nothing is recording', () => {
    flows.noteCommand('b-not-recording', 'navigate', { url: 'https://a.test/' });
    assert.equal(flows.isRecording('b-not-recording'), false);
  });

  it('keeps at most the step cap', async () => {
    const many = Array.from({ length: MAX_STEPS + 20 }, (_, i) => ({ id: `s${i}`, action: 'click', t: i }));
    const { dispatch } = recorderStub([many]);
    const snap = await flows.start(browserId, dispatch);
    assert.equal(snap.steps.length, MAX_STEPS);
  });

  it('stops with the final steps and keeps them readable', async () => {
    const { dispatch, modes } = recorderStub([[], [{ id: 'last', action: 'click' }]]);
    await flows.start(browserId, dispatch);
    const final = await flows.stop(browserId, dispatch);
    assert.equal(final.recording, false);
    assert.equal(final.steps[0].id, 'last');
    assert.deepEqual(modes, ['start', 'stop']);
    assert.equal((await flows.status(browserId, dispatch)).steps.length, 1, 'a stopped recording is not polled');
    assert.deepEqual(modes, ['start', 'stop']);
  });

  it('answers null when stopping a browser that never recorded', async () => {
    assert.equal(await flows.stop('b-never', recorderStub().dispatch), null);
  });

  it('stops on its own after the longest recording, then forgets it later', async () => {
    const { dispatch, modes } = recorderStub();
    await flows.start(browserId, dispatch);
    mock.timers.tick(RECORD_MAX_MS + RECORD_POLL_MS);
    await settle();
    assert.equal(modes.at(-1), 'stop');
    assert.equal(flows.isRecording(browserId), false);
    mock.timers.tick(RECORD_MAX_MS);
    assert.deepEqual(await flows.status(browserId), { recording: false, steps: [], secrets: [] });
  });

  it('refuses to discard a recording that is still running', async () => {
    await flows.start(browserId, recorderStub().dispatch);
    await assert.rejects(flows.discard(browserId), { status: 409 });
  });

  it('discards a stopped recording, and answers ok for one that does not exist', async () => {
    const { dispatch } = recorderStub([[{ id: 'a', action: 'click' }]]);
    await flows.start(browserId, dispatch);
    await flows.stop(browserId, dispatch);
    assert.deepEqual(await flows.discard(browserId), { ok: true });
    assert.deepEqual((await flows.status(browserId)).steps, []);
    assert.deepEqual(await flows.discard('b-never'), { ok: true });
  });

  it('starts afresh after a stopped recording', async () => {
    const first = recorderStub([[{ id: 'old', action: 'click' }]]);
    await flows.start(browserId, first.dispatch);
    await flows.stop(browserId, first.dispatch);
    const snap = await flows.start(browserId, recorderStub().dispatch);
    assert.deepEqual(snap.steps, []);
  });
});
