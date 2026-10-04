/**
 * Unit tests for cloud routines (src/modules/routines/cloud.ts): the server runs
 * a due cloud routine once across replicas, records the agent's answer, always
 * stops the browser it started, and records a run that could not happen as
 * failed. The cloud itself (start, connect, ask) is faked through `steps`.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-cloud-routines-');
const routines = await import('../../../../src/modules/routines/service.ts');
const cloud = await import('../../../../src/modules/routines/cloud.ts');
const { registry } = await import('../../../../src/modules/browsers/registry.ts');
const { control } = await import('../../../../src/modules/control/service.ts');
const { getConnection } = await import('../../../../src/platform/storage/index.ts');

const KEY = 'cloud-routines-key';
const MINUTE = 60_000;
const HOURLY = { name: 'Prices', prompt: 'Check prices', schedule: { kind: 'every', n: 1, unit: 'hours' } };
const CLOUD = { ...HOURLY, target: 'cloud' };

/** When a routine made now is first due. */
const due = () => Date.now() + 61 * MINUTE;

/** Runs the due routines and waits for the runs to end; answers the routine's latest run. */
async function runAndWait(id: string) {
  await Promise.all(await cloud.runDue(due()));
  return (await routines.list(KEY)).find((r) => r.id === id).runs[0];
}

describe('cloud routines', () => {
  let cancel;
  beforeEach(async () => {
    mock.method(registry, 'tell', () => 0);
    mock.method(control(), 'emit', async () => {});
    cancel = mock.method(control(), 'cancel', async () => {});
    await getConnection().delete('routines', {});
  });
  afterEach(() => mock.restoreAll());

  it('runs a due cloud routine on a browser it starts, records the answer and stops the browser', async () => {
    const made = await routines.create(KEY, CLOUD);
    mock.method(cloud.steps, 'launch', async () => 'b-cloud');
    mock.method(cloud.steps, 'connected', async () => {});
    const ask = mock.method(cloud.steps, 'ask', async () => ({ text: 'DONE: $5', toolCalls: [{ name: 'navigate' }] }));
    const run = await runAndWait(made.id);
    assert.deepEqual([run.status, run.result, run.steps], ['done', 'DONE: $5', ['navigate']]);
    assert.deepEqual(ask.mock.calls[0].arguments, [KEY, 'b-cloud', 'Check prices']);
    assert.deepEqual(cancel.mock.calls[0].arguments, [KEY, 'b-cloud']);
  });

  it('runs a due run once when replicas tick together', async () => {
    await routines.create(KEY, CLOUD);
    const launch = mock.method(cloud.steps, 'launch', async () => 'b-cloud');
    mock.method(cloud.steps, 'connected', async () => {});
    mock.method(cloud.steps, 'ask', async () => ({ text: 'DONE' }));
    const ticks = await Promise.all([cloud.runDue(due()), cloud.runDue(due())]);
    await Promise.all(ticks.flat());
    assert.equal(launch.mock.callCount(), 1);
  });

  it('leaves desktop routines to the desktops', async () => {
    await routines.create(KEY, HOURLY);
    assert.deepEqual(await cloud.runDue(due()), []);
  });

  it('records a run that could not start its browser as failed, saying why', async () => {
    const made = await routines.create(KEY, CLOUD);
    const run = await runAndWait(made.id);
    assert.equal(run.status, 'failed');
    assert.match(run.result, /^Error: Oya Cloud browsers are not set up/);
    assert.equal(cancel.mock.callCount(), 0, 'no browser to stop');
  });

  it('fails a run whose agent stopped to ask a person, and still stops the browser', async () => {
    const made = await routines.create(KEY, CLOUD);
    mock.method(cloud.steps, 'launch', async () => 'b-cloud');
    mock.method(cloud.steps, 'connected', async () => {});
    mock.method(cloud.steps, 'ask', async () => ({ text: 'NEEDS INPUT: solve the CAPTCHA' }));
    assert.equal((await runAndWait(made.id)).status, 'failed');
    assert.equal(cancel.mock.callCount(), 1);
  });

  it('records a browser that never connected as a failed run', async () => {
    const made = await routines.create(KEY, CLOUD);
    mock.method(cloud.steps, 'launch', async () => 'b-cloud');
    mock.method(cloud.steps, 'connected', async () => {
      throw new Error('The cloud browser did not connect in time.');
    });
    const run = await runAndWait(made.id);
    assert.deepEqual([run.status, run.result], ['failed', 'Error: The cloud browser did not connect in time.']);
    assert.equal(cancel.mock.callCount(), 1);
  });

  it('waits for the browser’s session to be ready, and fails when it ends first', async () => {
    const states = ['provisioning', 'ready'];
    mock.timers.enable({ apis: ['setTimeout'] });
    mock.method(control(), 'session', async () => ({ state: states.shift() }));
    const waiting = cloud.steps.connected(KEY, 'b-cloud');
    await new Promise((r) => setImmediate(r));
    mock.timers.tick(MINUTE);
    await waiting;
    mock.timers.reset();
    mock.method(control(), 'session', async () => ({ state: 'failed' }));
    await assert.rejects(cloud.steps.connected(KEY, 'b-cloud'), /failed before it connected/);
  });

  it('asks through the chat route with the project’s key, and turns its error into a thrown one', async () => {
    const fetch = mock.method(globalThis, 'fetch', async () => Response.json({ text: 'DONE' }));
    assert.deepEqual(await cloud.steps.ask(KEY, 'b 1', 'go'), { text: 'DONE' });
    const [url, init] = fetch.mock.calls[0].arguments;
    assert.match(String(url), /\/api\/browsers\/b%201\/chat$/);
    assert.equal(init.headers.authorization, `Bearer ${KEY}`);
    fetch.mock.mockImplementation(async () => Response.json({ error: 'Browser not connected', status: 404 }));
    await assert.rejects(cloud.steps.ask(KEY, 'b-1', 'go'), /Browser not connected/);
  });
});
