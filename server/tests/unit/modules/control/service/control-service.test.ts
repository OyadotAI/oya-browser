/**
 * Unit tests for ControlService's own reads: a project's sessions and event
 * log, scoped so one project never sees another's.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { projectId } from '../../../../../src/modules/control/service.ts';
import { readySession, scratchService } from '../../../support/control.ts';

const A = 'key-a',
  B = 'key-b';
let service;
beforeEach(async () => {
  service = scratchService();
  await readySession(service, A, 'mine');
  await service.reserve(A, { id: 'starting', provider: 'cdp' });
  await readySession(service, B, 'theirs');
});

describe('ControlService reads', () => {
  it('lists only the key’s own sessions, optionally by state, without private fields', async () => {
    assert.deepEqual((await service.sessions(A)).map((x) => x.id).sort(), ['mine', 'starting']);
    assert.deepEqual(
      (await service.sessions(A, ['ready'])).map((x) => x.id),
      ['mine'],
    );
    assert.equal('response' in (await service.sessions(A))[0], false);
  });

  it('finds its own session and answers null for another project’s', async () => {
    assert.equal((await service.findSession(A, 'mine')).id, 'mine');
    assert.equal(await service.findSession(A, 'theirs'), null);
    assert.equal(await service.findSession(A, 'missing'), null);
  });

  it('answers 404 for a session that is not the key’s', async () => {
    assert.equal((await service.session(A, 'mine')).state, 'ready');
    await assert.rejects(service.session(A, 'theirs'), { status: 404, code: 'not_found' });
  });

  it('reads only its own events, oldest first after the cursor', async () => {
    const events = await service.events(A);
    assert.ok(events.every((e) => e.project === projectId(A)));
    const after = await service.events(A, { after: events[0].id, limit: 1 });
    assert.deepEqual(
      after.map((e) => e.id),
      [events[1].id],
    );
  });

  it('names the project a key opens without touching storage', () => {
    assert.equal(service.projectIdFor(A), projectId(A));
  });
});
