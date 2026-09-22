/**
 * Unit tests for the one command path's vocabulary check: an action no
 * browser does answers 400, one this kind does not do answers 422 with where
 * it is supported, neither reaches the browser or its counters, the older
 * spellings still arrive, and an action the browser itself announced passes.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { sendCommand } from '../../../../../src/modules/browsers/connection/commands.ts';
import { registry } from '../../../../../src/modules/browsers/registry.ts';
import { scriptedBrowser } from '../../../support/agent.ts';
import { FakeSocket } from '../../../support/fakes.ts';
import { stubControl } from '../../../support/browsers.ts';

const CDP = 'b-vocab-cdp';
const OYA = 'b-vocab-oya';

describe('the command path checks the vocabulary before the browser is asked', () => {
  let cdp: ReturnType<typeof scriptedBrowser>;
  beforeEach(() => {
    stubControl();
    cdp = scriptedBrowser(CDP, 'k', () => ({ ok: true }));
  });
  afterEach(() => {
    mock.restoreAll();
    cdp.disconnect();
    registry.remove(OYA);
  });

  it('answers 400 action_unknown for an action no browser does, naming where to look', async () => {
    const err = await sendCommand(CDP, 'teleport').catch((e) => e);
    assert.deepEqual([err.status, err.code, err.action], [400, 'action_unknown', 'teleport']);
    assert.match(err.message, /^Unknown action "teleport"\. The browser detail lists/);
    assert.deepEqual(cdp.actions(), []);
  });

  it('answers 422 action_unsupported for an Oya-only action on a CDP browser, with where it is supported', async () => {
    const err = await sendCommand(CDP, 'read_console').catch((e) => e);
    assert.deepEqual(
      [err.status, err.code, err.clientType, err.supportedOn],
      [422, 'action_unsupported', 'cdp', ['oya']],
    );
    assert.match(err.message, /Supported on: oya\. Use an oya browser for this step, or pick another action\.$/);
    assert.deepEqual(cdp.actions(), []);
  });

  it('calls an action named like a lost connection unknown, not an outcome it cannot know', async () => {
    const err = await sendCommand(CDP, 'disconnect').catch((e) => e);
    assert.deepEqual([err.status, err.code], [400, 'action_unknown']);
  });

  it('leaves the browser’s counters untouched when it refuses', async () => {
    const before = registry.describe(CDP);
    await sendCommand(CDP, 'teleport').catch(() => {});
    const after = registry.describe(CDP);
    assert.deepEqual([after.commands, after.errors], [before.commands, before.errors]);
  });

  it('still takes the older spellings and sends them as written', async () => {
    await sendCommand(CDP, 'scroll-down', {});
    await sendCommand(CDP, 'press-key', { key: 'Enter' });
    assert.deepEqual(cdp.actions(), ['scroll-down', 'press-key']);
  });

  it('lets through an action a newer desktop announced, which this server does not know', async () => {
    const ws = new FakeSocket();
    registry.add(OYA, { ws, apiKey: 'k', name: 'D', clientType: 'oya', actions: ['click', 'teleport'] });
    const sent = sendCommand(OYA, 'teleport', {}, 30).catch((e) => e);
    await new Promise((r) => setImmediate(r));
    assert.equal(ws.sent.at(-1)?.action, 'teleport');
    await sent;
  });

  it('lists what the browser does on its detail: the CDP list, or what an Oya app announced', () => {
    assert.ok(registry.describe(CDP).actions.includes('back'));
    registry.add(OYA, { ws: new FakeSocket(), apiKey: 'k', name: 'D', clientType: 'oya' });
    assert.ok(registry.describe(OYA).actions.includes('workflow'));
    assert.ok(!registry.describe(OYA).actions.includes('evaluate_raw'));
  });
});
