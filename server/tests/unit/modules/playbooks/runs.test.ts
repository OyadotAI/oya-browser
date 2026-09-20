/**
 * Unit tests for background runs: their status as work succeeds or fails,
 * parking on a person until someone responds or the wait runs out, owner-only
 * access, and the events announced on the key's log.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const runs = await import('../../../../src/modules/playbooks/runs.ts');
const { control } = await import('../../../../src/modules/control/service.ts');
const { fingerprint } = await import('../../../../src/platform/audit.ts');
const { HUMAN_WAIT_MS } = await import('../../../../src/modules/playbooks/constants.ts');

const KEY = 'runs-key';
const OWNER = fingerprint(KEY);
let events: any[];

/** Lets the run's promise callbacks settle. */
const settle = () => new Promise((r) => setImmediate(r));

describe('background runs', () => {
  beforeEach(() => {
    events = [];
    mock.method(
      control(),
      'emit',
      async (key, type, browserId, detail) => void events.push({ key, type, browserId, detail }),
    );
  });
  afterEach(() => {
    mock.restoreAll();
    mock.timers.reset();
  });

  it('answers straight away with a running run, without its owner', () => {
    const run = runs.start(KEY, 'b-1', () => new Promise(() => {}));
    assert.match(run.id, /^run_/);
    assert.equal(run.status, 'running');
    assert.equal(run.browserId, 'b-1');
    assert.equal('owner' in run, false);
  });

  it('records the result when the work succeeds', async () => {
    const { id } = runs.start(KEY, 'b-1', async () => ({ text: 'DONE' }));
    await settle();
    const run = runs.get(OWNER, id);
    assert.equal(run.status, 'succeeded');
    assert.deepEqual(run.result, { text: 'DONE' });
    assert.ok(run.endedAt);
  });

  it('records the error and its status when the work fails, and announces it', async () => {
    const { id } = runs.start(KEY, 'b-1', async () => {
      throw Object.assign(new Error('Step 2 failed'), { status: 422 });
    });
    await settle();
    const run = runs.get(OWNER, id);
    assert.deepEqual([run.status, run.error, run.errorStatus], ['failed', 'Step 2 failed', 422]);
    assert.equal(events[0].type, 'run.failed');
    assert.deepEqual(events[0].detail, { runId: id, owner: OWNER, error: 'Step 2 failed' });
  });

  it('fails with 500 when the error carries no status', async () => {
    const { id } = runs.start(KEY, 'b-1', async () => {
      throw new Error('x');
    });
    await settle();
    assert.equal(runs.get(OWNER, id).errorStatus, 500);
  });

  it('parks on a person, announcing why, until they respond', async () => {
    let answered;
    const { id } = runs.start(KEY, 'b-1', async ({ requestHuman }) => {
      answered = await requestHuman({ reason: 'captcha', message: 'Solve it' });
      return { answered };
    });
    const parked = runs.get(OWNER, id);
    assert.equal(parked.status, 'needs_attention');
    assert.equal(parked.attention.reason, 'captcha');
    assert.equal(events[0].type, 'run.needs_attention');
    assert.equal(runs.respond(OWNER, id, 'solved'), true);
    await settle();
    assert.equal(answered, 'solved');
    assert.equal(runs.get(OWNER, id).status, 'succeeded');
  });

  it('answers "done" when the responder says nothing', async () => {
    let answered;
    const { id } = runs.start(KEY, 'b-1', async ({ requestHuman }) => (answered = await requestHuman({})));
    runs.respond(OWNER, id);
    await settle();
    assert.equal(answered, 'done');
  });

  it('refuses a response to a run that is not waiting, is unknown, or is another key’s', async () => {
    const { id } = runs.start(KEY, 'b-1', ({ requestHuman }) => requestHuman({}));
    assert.equal(runs.respond('someone-else', id), false);
    assert.equal(runs.respond(OWNER, 'run_unknown'), false);
    assert.equal(runs.respond(OWNER, id), true);
    assert.equal(runs.respond(OWNER, id), false, 'already answered');
  });

  it('hides a run from another key', () => {
    const { id } = runs.start(KEY, 'b-1', () => new Promise(() => {}));
    assert.equal(runs.get('someone-else', id), null);
    assert.equal(runs.get(OWNER, 'run_unknown'), null);
  });

  it('fails the wait after 30 minutes with nobody answering', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    const { id } = runs.start(KEY, 'b-1', ({ requestHuman }) => requestHuman({ reason: 'login' }));
    mock.timers.tick(HUMAN_WAIT_MS);
    await settle();
    const run = runs.get(OWNER, id);
    assert.equal(run.status, 'failed');
    assert.equal(run.error, 'Nobody responded within 30 minutes');
  });

  it('never fails a run because its announcement could not be recorded', async () => {
    mock.restoreAll();
    mock.method(control(), 'emit', async () => {
      throw new Error('store down');
    });
    const error = mock.method(console, 'error', () => {});
    const { id } = runs.start(KEY, 'b-1', async ({ requestHuman }) => requestHuman({}));
    await settle();
    assert.equal(runs.get(OWNER, id).status, 'needs_attention');
    assert.match(error.mock.calls[0].arguments[0], /run\.needs_attention not recorded/);
    runs.respond(OWNER, id);
  });
});
