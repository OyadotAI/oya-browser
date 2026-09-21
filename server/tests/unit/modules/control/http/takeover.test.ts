/**
 * Unit tests for transferControl, POST /control/sessions/:id/control: the
 * caller's credential is the holder, acquiring waits for in-flight commands to
 * settle, and every other action refuses at once while they are running.
 */
import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../../support/data-dir.ts';

ownDataDir('oya-control-http-takeover-');
const { control, hash } = await import('../../../../../src/modules/control/service.ts');
const { transferControl } = await import('../../../../../src/modules/control/http/takeover.ts');
const { registry } = await import('../../../../../src/modules/browsers/registry.ts');
const { connectBrowser, disconnectBrowser } = await import('../../../support/fakes.ts');
const { readySession } = await import('../../../support/control.ts');

let n = 0,
  id;
beforeEach(async () => {
  id = `t-${n++}`;
  await readySession(control(), 'key-a', id);
  connectBrowser(id);
});
afterEach(() => disconnectBrowser(id));

/** A control request from the holder of `token`. */
const request = (action, token = 'oya_alice', extra = {}) => ({
  authToken: token,
  params: { id },
  body: { action, ...extra },
});

describe('transferControl', () => {
  it('applies the action with the caller’s credential hash as the holder', async () => {
    const state = await transferControl('key-a', request('acquire'));
    assert.deepEqual([state.mode, state.holder], ['human', hash('oya_alice')]);
  });

  it('lets a member renew their hold after their console credential was renewed', async () => {
    const principal = { project: 'p1', memberUser: 'u1' };
    await transferControl('key-a', { ...request('acquire', 'oya_first'), principal });
    const state = await transferControl('key-a', { ...request('renew', 'oya_renewed'), principal });
    assert.equal(state.mode, 'human');
  });

  it('passes force through', async () => {
    await transferControl('key-a', request('acquire'));
    await assert.rejects(transferControl('key-a', request('acquire', 'oya_bob')), { code: 'control_busy' });
    const state = await transferControl('key-a', request('acquire', 'oya_bob', { force: true }));
    assert.equal(state.holder, hash('oya_bob'));
  });

  it('refuses other actions at once while commands are in flight', async () => {
    registry.get(id).pending = 1;
    await assert.rejects(transferControl('key-a', request('request')), { code: 'commands_pending' });
  });

  it('renews a hold while the holder’s own commands are in flight', async () => {
    await transferControl('key-a', request('acquire'));
    registry.get(id).pending = 1;
    assert.equal((await transferControl('key-a', request('renew'))).mode, 'human');
  });

  it('waits for in-flight commands to settle before acquiring', async () => {
    registry.get(id).pending = 1;
    setTimeout(() => (registry.get(id).pending = 0), 50);
    assert.equal((await transferControl('key-a', request('acquire'))).mode, 'human');
  });

  it('answers 404 for another project’s session', async () => {
    await assert.rejects(transferControl('key-b', request('acquire')), { status: 404 });
  });
});
