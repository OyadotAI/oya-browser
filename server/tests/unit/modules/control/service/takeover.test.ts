/**
 * Unit tests for human takeover: request, acquire, renew, release, resume and
 * return; busy refusals between operators and force; revisions; and the command
 * gate that admits agent commands only while the agent has control.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { readySession, scratchService } from '../../../support/control.ts';

const A = 'key-a',
  B = 'key-b',
  NOW = 1_700_000_000_000;
let service;
beforeEach(async () => {
  mock.timers.enable({ apis: ['Date'], now: NOW });
  service = scratchService();
  await readySession(service, A, 's');
});
afterEach(() => mock.timers.reset());

/** Applies `action` to session s as `holder`. */
const act = (action, holder = 'alice', options = {}) => service.takeover(A, 's', action, holder, options);

describe('takeover actions', () => {
  it('request pauses the agent briefly while the operator’s page connects', async () => {
    const c = await act('request');
    assert.deepEqual(
      { ...c, revision: undefined },
      { mode: 'paused', holder: 'alice', takeover: true, expiresAt: NOW + 10_000, revision: undefined },
    );
    assert.equal((await service.events(A)).at(-1).type, 'control.paused');
  });

  it('acquire gives the operator human control for five minutes', async () => {
    const c = await act('acquire');
    assert.deepEqual([c.mode, c.holder, c.expiresAt], ['human', 'alice', NOW + 300_000]);
    assert.equal((await service.events(A)).at(-1).type, 'control.human');
  });

  it('refuses another operator while the hold is live, unless they force it', async () => {
    await act('acquire');
    await assert.rejects(act('acquire', 'bob'), { status: 409, code: 'control_busy' });
    await assert.rejects(act('request', 'bob'), { code: 'control_busy' });
    assert.equal((await act('acquire', 'bob', { force: true })).holder, 'bob');
  });

  it('refuses another operator mid-takeover', async () => {
    await act('request');
    await assert.rejects(act('acquire', 'bob'), {
      code: 'control_busy',
      message: 'Another operator is taking control',
    });
  });

  it('refuses to acquire while agent commands are in flight', async () => {
    await service.beginCommand('s');
    await assert.rejects(act('acquire'), { code: 'commands_pending' });
  });

  it('renew extends only the holder’s live hold', async () => {
    await act('acquire');
    mock.timers.tick(1000);
    assert.equal((await act('renew')).expiresAt, NOW + 301_000);
    await assert.rejects(act('renew', 'bob'), { code: 'control_busy' });
    mock.timers.tick(300_000);
    await assert.rejects(act('renew'), { message: 'Human control has expired or changed' });
  });

  it('release leaves the agent paused, and only the holder may release', async () => {
    await act('acquire');
    await assert.rejects(act('release', 'bob'), { code: 'control_busy' });
    assert.deepEqual({ ...(await act('release')), revision: 0 }, { mode: 'paused', revision: 0 });
  });

  it('resume hands control back to the agent once no human holds it', async () => {
    await act('acquire');
    await assert.rejects(act('resume'), { message: 'Human control must be released first' });
    await act('release');
    assert.equal((await act('resume')).mode, 'agent');
  });

  it('return gives control back to the agent, only from the current operator', async () => {
    await act('acquire');
    await assert.rejects(act('return', 'bob'), { message: 'Only the current operator can return control' });
    assert.equal((await act('return')).mode, 'agent');
  });

  it('return is a no-op without an event while the agent already has control', async () => {
    const before = (await service.events(A)).length;
    assert.deepEqual(await act('return'), { mode: 'agent' });
    assert.equal((await service.events(A)).length, before);
  });

  it('stamps each change with a strictly increasing revision', async () => {
    const first = (await act('request')).revision;
    const second = (await act('acquire')).revision;
    assert.ok(first >= NOW);
    assert.ok(second > first);
  });

  it('refuses an unknown action with 400', async () => {
    await assert.rejects(act('grab'), { status: 400, code: 'invalid_action' });
    await assert.rejects(act('toString'), { status: 400 });
  });

  it('refuses a session that is not ready', async () => {
    await service.reserve(A, { id: 'p', provider: 'cdp' });
    await assert.rejects(service.takeover(A, 'p', 'acquire', 'alice'), { code: 'not_ready' });
  });

  it('answers 404 for another project’s session', async () => {
    await assert.rejects(service.takeover(B, 's', 'acquire', 'mallory'), { status: 404 });
  });
});

describe('beginCommand', () => {
  /** The session's in-flight count. */
  const inFlight = async () => (await service.store.get('session', 's')).inFlight;

  it('admits an agent command and settles it once, however often settle is called', async () => {
    const settle = await service.beginCommand('s');
    assert.equal(await inFlight(), 1);
    await settle();
    await settle();
    assert.equal(await inFlight(), 0);
  });

  it('pauses agent commands while a human holds the browser, but admits the holder’s own', async () => {
    await act('acquire');
    await assert.rejects(service.beginCommand('s'), { code: 'control_paused' });
    const settle = await service.beginCommand('s', 'alice');
    await settle();
  });

  it('lets commands for an unknown session through ungated', async () => {
    const settle = await service.beginCommand('nobody');
    await settle();
  });
});
