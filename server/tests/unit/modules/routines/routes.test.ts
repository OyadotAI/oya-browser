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
});
