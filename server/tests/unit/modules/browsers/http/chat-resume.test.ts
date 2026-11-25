/**
 * The desktop's two calls about a chat run that outlives its request: resume
 * collects the run's answer, stop ends the run, and both answer for a run on
 * this server whether or not the browser's socket is connected right now.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { resumeChat, stopChat } from '../../../../../src/modules/browsers/http/chat-resume.ts';
import { resumableRuns, runKey } from '../../../../../src/modules/agent/chat.ts';
import { FakeResponse, fakeRequest } from '../../../support/browsers.ts';

/** A browser id nothing is connected under. */
const B = 'b-resume';

/** Starts a run for key-a in B's lane `lane`; returns its signal and a way to end it. */
function startRun(lane: string, token = 'run-1') {
  const out: { signal?: AbortSignal; resolve?: (v: unknown) => void } = {};
  const work = (signal: AbortSignal) => new Promise((resolve) => Object.assign(out, { signal, resolve }));
  resumableRuns.release(resumableRuns.start(runKey(B, lane), 'key-a', token, work));
  return out;
}

/** Calls a route on B with `body`, as `key`. */
async function call(route: (req, res) => unknown, body: object, key = 'key-a') {
  const res = new FakeResponse();
  await route(fakeRequest({ key, params: { browserId: B }, body }), res);
  return res;
}

describe('chat resume and stop', () => {
  afterEach(() => mock.restoreAll());

  it('resume answers the run its caller started, with the browser not connected', async () => {
    const run = startRun('tab-1');
    const waiting = call(resumeChat, { lane: 'tab-1', resume: 'run-1' });
    run.resolve({ text: 'the answer' });
    const res = await waiting;
    assert.equal(res.statusCode, 200);
    assert.deepEqual(JSON.parse(res.ended), { text: 'the answer' });
  });

  it('resume with no such run is a 404 with a code, before any 200', async () => {
    const res = await call(resumeChat, { lane: 'tab-none', resume: 'run-1' });
    assert.equal(res.statusCode, 404);
    assert.equal(res.body.code, 'chat_run_gone');
    assert.equal(res.ended, null);
  });

  it("resume refuses another key, and another run's token", async () => {
    const run = startRun('tab-2');
    assert.equal((await call(resumeChat, { lane: 'tab-2', resume: 'run-1' }, 'key-b')).statusCode, 404);
    assert.equal((await call(resumeChat, { lane: 'tab-2', resume: 'stale' })).statusCode, 404);
    run.resolve('x');
  });

  it('resume and stop refuse a body without a usable lane or token', async () => {
    assert.equal((await call(resumeChat, { lane: 'bad lane', resume: 'run-1' })).statusCode, 400);
    assert.equal((await call(resumeChat, { lane: 'tab-1' })).statusCode, 400);
    assert.equal((await call(stopChat, {})).statusCode, 400);
    assert.equal((await call(stopChat, { lane: 'tab-1' })).statusCode, 400);
  });

  it("stop ends the caller's run at once, with the browser not connected", async () => {
    const run = startRun('tab-3');
    const res = await call(stopChat, { lane: 'tab-3', resume: 'run-1' });
    assert.deepEqual(res.body, { ok: true, stopped: true });
    assert.equal(run.signal.aborted, true);
    run.resolve('x');
  });

  it("stop for another key's run stops nothing", async () => {
    const run = startRun('tab-4');
    const res = await call(stopChat, { lane: 'tab-4', resume: 'run-1' }, 'key-b');
    assert.deepEqual(res.body, { ok: true, stopped: false });
    assert.equal(run.signal.aborted, false);
    resumableRuns.stop(runKey(B, 'tab-4'), 'key-a', 'run-1');
    run.resolve('x');
  });

  it('a stop naming an earlier run leaves the run in the lane alone', async () => {
    const run = startRun('tab-5', 'run-2');
    const res = await call(stopChat, { lane: 'tab-5', resume: 'run-1' });
    assert.deepEqual(res.body, { ok: true, stopped: false });
    assert.equal(run.signal.aborted, false);
    resumableRuns.stop(runKey(B, 'tab-5'), 'key-a', 'run-2');
    run.resolve('x');
  });
});
