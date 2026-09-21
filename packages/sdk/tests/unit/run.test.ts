/**
 * Unit tests for Run and its polling loop (src/run.ts, src/run-watch.ts):
 * callbacks fire in order and once, transient poll errors are tolerated, and
 * `done` settles with the outcome.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { client, flush, tickUntil, type Route } from './support/fake-fetch.ts';

const POLL = 'GET /api/runs/r1';
const running = { body: { id: 'r1', status: 'running', attention: null } };

/** Submits a prompt on browser b1 and returns the run with its recorded calls. */
async function submit(poll: Route | Route[], options: Record<string, unknown> = {}) {
  const { oya, calls } = client({
    'GET /api/browsers/b1': { body: { id: 'b1' } },
    'POST /api/browsers/b1/runs': { body: { id: 'r1' } },
    'POST /api/runs/r1/respond': { body: {} },
    [POLL]: poll,
  });
  const b = await oya.browser.get('b1');
  return { run: await b.submit({ prompt: 'go' }, { pollMs: 1000, ...options }), calls };
}

describe('Run', () => {
  beforeEach(() => mock.timers.enable({ apis: ['setTimeout'] }));
  afterEach(() => mock.timers.reset());

  it('sends the task and inputs, but not the callbacks', async () => {
    const { run, calls } = await submit(
      { body: { id: 'r1', status: 'succeeded', result: {} } },
      { data: { a: 1 }, onSuccess: () => {} },
    );
    await run.done;
    assert.deepEqual(calls[1].body, { prompt: 'go', data: { a: 1 } });
  });

  it('polls until success, then fires onHealed before onSuccess', async () => {
    const order: string[] = [];
    const result = { healed: true, draft: 'x:draft' };
    const { run } = await submit([running, { body: { id: 'r1', status: 'succeeded', result } }], {
      onHealed: () => order.push('healed'),
      onSuccess: () => order.push('success'),
    });
    await tickUntil(run.done, 1000);
    assert.deepEqual(await run.done, result);
    assert.deepEqual(order, ['healed', 'success']);
  });

  it('rejects done and fires onFailure with the run error and status', async () => {
    let failure: unknown;
    const { run } = await submit(
      { body: { id: 'r1', status: 'failed', error: 'quota', errorStatus: 429 } },
      { onFailure: (e: unknown) => (failure = e) },
    );
    await assert.rejects(run.done, { status: 429, message: 'quota' });
    assert.equal((failure as Error).message, 'quota');
  });

  it('defaults a failure without a status to 500', async () => {
    const { run } = await submit({ body: { id: 'r1', status: 'failed' } });
    await assert.rejects(run.done, { status: 500, message: 'Run failed' });
  });

  it('survives four failed polls in a row', async () => {
    const down = { status: 503, body: { error: 'down' } };
    const { run } = await submit([down, down, down, down, { body: { id: 'r1', status: 'succeeded' } }]);
    await tickUntil(run.done, 1000);
    assert.deepEqual(await run.done, {});
  });

  it('gives up after the fifth failed poll in a row', async () => {
    const { run } = await submit({ status: 503, body: { error: 'down' } });
    await tickUntil(run.done, 1000);
    await assert.rejects(run.done, { status: 503, message: 'down' });
  });

  it('fires onHumanAttention once per request, with an absolute link and respond()', async () => {
    const attention = { id: 'a1', reason: 'captcha', message: 'solve', liveViewUrl: '/live/b1', at: 1 };
    const waiting = { body: { id: 'r1', status: 'needs_attention', attention } };
    const seen: Array<{ liveViewUrl?: string; respond(r?: string): Promise<void> }> = [];
    const { run, calls } = await submit([waiting, waiting, { body: { id: 'r1', status: 'succeeded' } }], {
      onHumanAttention: (req: (typeof seen)[number]) => seen.push(req),
    });
    await tickUntil(run.done, 1000);
    assert.equal(seen.length, 1);
    assert.equal(seen[0].liveViewUrl, 'https://oya.test/live/b1');
    await seen[0].respond();
    assert.deepEqual(calls.at(-1)?.body, { response: 'done' });
  });

  it('keeps going when a callback throws', async () => {
    mock.method(console, 'error', () => {});
    const { run } = await submit(
      { body: { id: 'r1', status: 'succeeded' } },
      {
        onSuccess: () => {
          throw new Error('boom');
        },
      },
    );
    assert.deepEqual(await run.done, {});
  });

  it('does not leave an unhandled rejection when only onFailure is used', async () => {
    const { run } = await submit({ body: { id: 'r1', status: 'failed' } }, { onFailure: () => {} });
    await flush();
    await assert.rejects(run.done);
  });
});
