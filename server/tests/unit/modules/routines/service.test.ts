/**
 * Unit tests for a project's routines (src/modules/routines): kept per project
 * and sealed, edited without losing history, claimed by exactly one browser per
 * due run, finished, cleared and imported from a desktop's local list. Storage
 * is the scratch SQLite database; the push to desktops is faked.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-routines-');
const routines = await import('../../../../src/modules/routines/service.ts');
const { registry } = await import('../../../../src/modules/browsers/registry.ts');
const { getConnection } = await import('../../../../src/platform/storage/index.ts');
const { fingerprint } = await import('../../../../src/platform/audit.ts');
const { ROUTINE_RUN_LEASE_MS, ROUTINE_RUNS_KEPT } = await import('../../../../src/modules/routines/constants.ts');

const KEY = 'routines-key';
const OTHER = 'routines-other-key';
const HOURLY = { name: 'Check prices', prompt: 'Look up the price', schedule: { kind: 'every', n: 1, unit: 'hours' } };

describe('routines', () => {
  let tell;
  beforeEach(async () => {
    tell = mock.method(registry, 'tell', () => 0);
    await getConnection().delete('routines', {});
  });
  afterEach(() => mock.restoreAll());

  it('keeps a project’s routines to that project, and tells its desktops about a change', async () => {
    const made = await routines.create(KEY, HOURLY);
    assert.deepEqual(
      (await routines.list(KEY)).map((r) => r.name),
      ['Check prices'],
    );
    assert.deepEqual(await routines.list(OTHER), []);
    assert.deepEqual(tell.mock.calls[0].arguments, [KEY, { type: 'routines_changed' }]);
    assert.equal(made.enabled, true);
    assert.equal(made.lastRunAt, null);
  });

  it('stores routines sealed, never the prompt in the clear', async () => {
    await routines.create(KEY, HOURLY);
    const [row] = await getConnection().select('routines', { owner: fingerprint(KEY) });
    assert.ok(!String(row.value).includes('Look up the price'));
  });

  it('refuses a routine without a name, a prompt or a valid schedule, saying what is wrong', async () => {
    await assert.rejects(routines.create(KEY, { ...HOURLY, name: ' ' }), { status: 400, message: /name/ });
    await assert.rejects(routines.create(KEY, { ...HOURLY, schedule: { kind: 'every', n: 0, unit: 'hours' } }), {
      status: 400,
    });
    await assert.rejects(routines.create(KEY, { ...HOURLY, schedule: { kind: 'daily', at: '25:00' } }), {
      status: 400,
      message: /HH:MM/,
    });
  });

  it('edits only the fields sent, keeping the history; turning it off is one field', async () => {
    const made = await routines.create(KEY, HOURLY);
    await routines.claim(KEY, made.id, { lastRunAt: null, runId: 'r1' });
    const off = await routines.edit(KEY, made.id, { enabled: false });
    assert.equal(off.enabled, false);
    assert.equal(off.prompt, HOURLY.prompt);
    assert.equal(off.runs.length, 1);
    await assert.rejects(routines.edit(KEY, 'nope', { enabled: true }), { status: 404 });
  });

  it('gives a due run to exactly one of two browsers claiming it at once', async () => {
    const made = await routines.create(KEY, HOURLY);
    const claims = await Promise.allSettled([
      routines.claim(KEY, made.id, { lastRunAt: null, runId: 'a', browserId: 'desktop-a' }),
      routines.claim(KEY, made.id, { lastRunAt: null, runId: 'b', browserId: 'desktop-b' }),
    ]);
    assert.equal(claims.filter((c) => c.status === 'fulfilled').length, 1);
    assert.equal((claims.find((c) => c.status === 'rejected') as PromiseRejectedResult).reason.status, 409);
    const [stored] = await routines.list(KEY);
    assert.equal(stored.runs.length, 1);
    assert.equal(stored.runs[0].status, 'running');
  });

  it('refuses a claim on a run another browser already started, or one still running', async () => {
    const made = await routines.create(KEY, HOURLY);
    await routines.claim(KEY, made.id, { lastRunAt: null, runId: 'a' });
    await assert.rejects(routines.claim(KEY, made.id, { lastRunAt: null, runId: 'b' }), { status: 409 });
    const [running] = await routines.list(KEY);
    await assert.rejects(routines.claim(KEY, made.id, { lastRunAt: running.lastRunAt, runId: 'c' }), {
      status: 409,
      message: /already running/,
    });
  });

  it('lets a run whose browser died be claimed again once its lease is up, marking it interrupted', async () => {
    const made = await routines.create(KEY, HOURLY);
    const first = await routines.claim(KEY, made.id, { lastRunAt: null, runId: 'a' });
    mock.timers.enable({ apis: ['Date'], now: first.lastRunAt! + ROUTINE_RUN_LEASE_MS });
    const again = await routines.claim(KEY, made.id, { lastRunAt: first.lastRunAt, runId: 'b' });
    assert.deepEqual(
      again.runs.map((r) => [r.id, r.status]),
      [
        ['b', 'running'],
        ['a', 'interrupted'],
      ],
    );
    mock.timers.reset();
  });

  it('records how a run ended, bounded, and refuses an unknown run or status', async () => {
    const made = await routines.create(KEY, HOURLY);
    await routines.claim(KEY, made.id, { lastRunAt: null, runId: 'a' });
    const done = await routines.finishRun(KEY, made.id, 'a', { status: 'done', result: 'ok', steps: ['click', 7] });
    assert.deepEqual([done.runs[0].status, done.runs[0].result, done.runs[0].steps], ['done', 'ok', ['click']]);
    await assert.rejects(routines.finishRun(KEY, made.id, 'zz', { status: 'done' }), { status: 404 });
    await assert.rejects(routines.finishRun(KEY, made.id, 'a', { status: 'running' }), { status: 400 });
  });

  it('keeps both a finish and an edit made at the same moment, instead of one overwriting the other', async () => {
    const made = await routines.create(KEY, HOURLY);
    await routines.claim(KEY, made.id, { lastRunAt: null, runId: 'a' });
    await Promise.all([
      routines.finishRun(KEY, made.id, 'a', { status: 'done' }),
      routines.edit(KEY, made.id, { name: 'Renamed' }),
    ]);
    const [after] = await routines.list(KEY);
    assert.deepEqual([after.name, after.runs[0].status], ['Renamed', 'done']);
  });

  it('clears finished runs and keeps one in progress', async () => {
    const made = await routines.create(KEY, HOURLY);
    await routines.claim(KEY, made.id, { lastRunAt: null, runId: 'a' });
    await routines.finishRun(KEY, made.id, 'a', { status: 'done' });
    const [ran] = await routines.list(KEY);
    await routines.claim(KEY, made.id, { lastRunAt: ran.lastRunAt, runId: 'b' });
    const cleared = await routines.clearRuns(KEY, made.id);
    assert.deepEqual(
      cleared.runs.map((r) => r.id),
      ['b'],
    );
  });

  it('deletes a routine, and says so for one that is gone', async () => {
    const made = await routines.create(KEY, HOURLY);
    await routines.remove(KEY, made.id);
    assert.deepEqual(await routines.list(KEY), []);
    await assert.rejects(routines.remove(KEY, made.id), { status: 404 });
  });

  it('imports a desktop’s local routines with their ids and history, once, marking an unfinished run interrupted', async () => {
    const local = {
      id: 'local-1',
      createdAt: 1000,
      lastRunAt: 2000,
      ...HOURLY,
      runs: [{ id: 'r', startedAt: 2000, status: 'running' }, { bad: true }],
    };
    assert.equal(await routines.importRoutines(KEY, [local]), 1);
    assert.equal(await routines.importRoutines(KEY, [local]), 0, 'a second import changes nothing');
    const [kept] = await routines.list(KEY);
    assert.deepEqual([kept.id, kept.createdAt, kept.lastRunAt], ['local-1', 1000, 2000]);
    assert.deepEqual(
      kept.runs.map((r) => r.status),
      ['interrupted'],
    );
    await assert.rejects(routines.importRoutines(KEY, 'nope'), { status: 400 });
  });

  it('keeps only the newest runs', async () => {
    const made = await routines.create(KEY, HOURLY);
    let last = null;
    for (let i = 0; i <= ROUTINE_RUNS_KEPT; i++) {
      const claimed = await routines.claim(KEY, made.id, { lastRunAt: last, runId: `r${i}` });
      await routines.finishRun(KEY, made.id, `r${i}`, { status: 'done' });
      last = claimed.lastRunAt;
    }
    const [kept] = await routines.list(KEY);
    assert.equal(kept.runs.length, ROUTINE_RUNS_KEPT);
    assert.equal(kept.runs[0].id, `r${ROUTINE_RUNS_KEPT}`);
  });
});
