/**
 * Unit tests for lease renewal: this replica extends the leases of sessions
 * attached to it when they run low, ends expired human control, and tells each
 * connected browser its control mode.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../../support/data-dir.ts';

ownDataDir('oya-control-leases-');
const { control } = await import('../../../../../src/modules/control/service.ts');
const { renewLeases } = await import('../../../../../src/modules/control/worker/leases.ts');
const { connectBrowser, disconnectBrowser } = await import('../../../support/fakes.ts');
const { patchRow, readySession } = await import('../../../support/control.ts');

const NOW = 1_700_000_000_000;
let n = 0,
  id;
beforeEach(async () => {
  mock.timers.enable({ apis: ['Date'], now: NOW });
  id = `s${n++}`;
  await readySession(control(), 'key-a', id);
});
afterEach(() => {
  disconnectBrowser(id);
  mock.timers.reset();
});
const stored = () => control().store.get('session', id);

describe('renewLeases', () => {
  it('extends a held session’s lease once it runs low', async () => {
    connectBrowser(id);
    await patchRow(control(), 'session', id, { leaseUntil: NOW + 10_000 });
    await renewLeases();
    assert.equal((await stored()).leaseUntil, NOW + 30_000);
  });

  it('writes nothing while the lease has time left', async () => {
    connectBrowser(id);
    await patchRow(control(), 'session', id, { leaseUntil: NOW + 25_000 });
    await renewLeases();
    assert.equal((await stored()).leaseUntil, NOW + 25_000);
  });

  it('never renews a session that is not attached here', async () => {
    await patchRow(control(), 'session', id, { leaseUntil: NOW + 10_000 });
    await renewLeases();
    assert.equal((await stored()).leaseUntil, NOW + 10_000);
  });

  it('never renews a session another replica owns', async () => {
    connectBrowser(id);
    await patchRow(control(), 'session', id, { instance: 'other', leaseUntil: NOW + 10_000 });
    await renewLeases();
    assert.equal((await stored()).leaseUntil, NOW + 10_000);
  });

  it('pauses an expired human takeover and tells the browser its control mode', async () => {
    const ws = connectBrowser(id);
    await control().takeover('key-a', id, 'acquire', 'alice');
    mock.timers.tick(300_000);
    await renewLeases();
    assert.equal((await stored()).control.mode, 'paused');
    assert.equal((await control().events('key-a')).at(-1).type, 'control.paused');
    const [message] = ws.ofType('control_mode');
    assert.equal(message.mode, 'paused');
    assert.equal(message.state.mine, false);
  });

  it('tells a connected browser its mode on every round', async () => {
    const ws = connectBrowser(id);
    await renewLeases();
    assert.equal(ws.ofType('control_mode')[0].mode, 'agent');
  });
});
