/**
 * Unit tests for the routine routes: a key reads and changes only its own
 * routines, and a losing claim answers 409.
 */
import { describe, it, before, after, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-routine-routes-');
const { router } = await import('../../../../src/modules/routines/routes.ts');
const { allowKey, callRoute } = await import('../../support/agent.ts');
const { registry } = await import('../../../../src/modules/browsers/registry.ts');
const cloud = await import('../../../../src/modules/routines/cloud.ts');

const KEY = 'routine-routes-key';
const HOURLY = { name: 'Prices', prompt: 'Check', schedule: { kind: 'every', n: 1, unit: 'hours' } };

describe('routine routes', () => {
  let forget: () => void;
  before(() => {
    forget = allowKey(KEY);
    mock.method(registry, 'tell', () => 0);
  });
  after(() => (forget(), mock.restoreAll()));

  /** Calls the router as the test key. */
  const call = (method: string, url: string, body?: unknown) => callRoute(router, { method, url, key: KEY, body });

  it('refuses a caller without an API key', async () => {
    assert.equal((await callRoute(router, { url: '/routines' })).status, 401);
  });

  it('creates, lists, turns off, claims, finishes and deletes a routine', async () => {
    const made = await call('POST', '/routines', HOURLY);
    assert.equal(made.status, 201);
    const id = made.body.id;
    assert.equal((await call('GET', '/routines')).body.routines[0].id, id);
    assert.equal((await call('PATCH', `/routines/${id}`, { enabled: false })).body.enabled, false);
    assert.equal((await call('POST', `/routines/${id}/claim`, { lastRunAt: null, runId: 'r1' })).status, 200);
    assert.equal((await call('POST', `/routines/${id}/claim`, { lastRunAt: null, runId: 'r2' })).status, 409);
    assert.equal((await call('PATCH', `/routines/${id}/runs/r1`, { status: 'done' })).body.runs[0].status, 'done');
    assert.deepEqual((await call('DELETE', `/routines/${id}/runs`)).body.runs, []);
    assert.equal((await call('DELETE', `/routines/${id}`)).status, 200);
    assert.equal((await call('DELETE', `/routines/${id}`)).status, 404);
  });

  it('imports a desktop’s routines', async () => {
    const res = await call('POST', '/routines/import', { routines: [{ id: 'l1', ...HOURLY }] });
    assert.deepEqual(res.body, { imported: 1 });
  });

  it('lists each routine with when it runs next, null when it is off', async () => {
    const on = (await call('POST', '/routines', HOURLY)).body;
    const off = (await call('POST', '/routines', { ...HOURLY, name: 'Off', enabled: false })).body;
    assert.equal(on.nextRunAt, on.createdAt + 60 * 60_000);
    const listed = (await call('GET', '/routines')).body.routines;
    assert.equal(listed.find((r) => r.id === off.id).nextRunAt, null);
    assert.equal(listed.find((r) => r.id === on.id).nextRunAt, on.nextRunAt);
  });

  it('runs a cloud routine now with 202, and answers 404 for another key’s routine', async () => {
    mock.method(cloud.steps, 'launch', async () => 'b-cloud');
    mock.method(cloud.steps, 'connected', async () => {});
    mock.method(cloud.steps, 'ask', () => new Promise(() => {}));
    const made = (await call('POST', '/routines', { ...HOURLY, target: 'cloud' })).body;
    const started = await call('POST', `/routines/${made.id}/run`);
    assert.deepEqual([started.status, started.body.runs[0].status], [202, 'running']);
    const stranger = 'routine-routes-other-key';
    const forgetStranger = allowKey(stranger);
    const res = await callRoute(router, { method: 'POST', url: `/routines/${made.id}/run`, key: stranger });
    forgetStranger();
    assert.equal(res.status, 404);
  });

  it('keeps a playbook routine’s values and profile, refusing another key’s profile and resetting to the default with null', async () => {
    const { container } = await import('../../../../src/app/container.ts');
    const mine = container.personas.create(KEY, { name: 'mine' });
    const stranger = 'routine-routes-profile-stranger';
    const forgetStranger = allowKey(stranger);
    const theirs = container.personas.create(stranger, { name: 'theirs' });
    const body = { ...HOURLY, prompt: '', playbook: 'search', values: { q: 'mug' }, persona: mine.id };
    const made = await call('POST', '/routines', body);
    assert.deepEqual([made.status, made.body.values, made.body.persona], [201, { q: 'mug' }, mine.id]);
    assert.equal((await call('POST', '/routines', { ...body, persona: theirs.id })).status, 404);
    assert.equal((await call('PATCH', `/routines/${made.body.id}`, { persona: null })).body.persona, undefined);
    forgetStranger();
  });

  it('still turns off a routine whose profile was deleted, and checks only a profile given anew', async () => {
    const { container } = await import('../../../../src/app/container.ts');
    const gone = container.personas.create(KEY, { name: 'gone' });
    const made = await call('POST', '/routines', { ...HOURLY, persona: gone.id });
    container.personas.remove(KEY, gone.id);
    const off = await call('PATCH', `/routines/${made.body.id}`, { enabled: false });
    assert.deepEqual([off.status, off.body.enabled], [200, false]);
    assert.equal((await call('PATCH', `/routines/${made.body.id}`, { persona: gone.id })).status, 404);
  });
});
