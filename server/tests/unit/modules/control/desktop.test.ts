/**
 * Unit tests for desktop takeover: the control state as the desktop sees it,
 * and getting, requesting, acquiring (waiting for the agent's command),
 * renewing and returning control of a browser.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-control-desktop-');
const { control } = await import('../../../../src/modules/control/service.ts');
const { desktopControl, desktopHolder, desktopState } = await import('../../../../src/modules/control/desktop.ts');
const { registry } = await import('../../../../src/modules/browsers/registry.ts');
const { connectBrowser, disconnectBrowser } = await import('../../support/fakes.ts');
const { readySession } = await import('../../support/control.ts');
const { advance } = await import('../../support/http.ts');

let n = 0,
  id;
beforeEach(async () => {
  id = `d-${n++}`;
  await readySession(control(), 'key-a', id);
  connectBrowser(id);
});
afterEach(() => {
  disconnectBrowser(id);
  mock.timers.reset();
});

describe('desktopState', () => {
  it('says whether the desktop holds control, and whether a takeover is under way', () => {
    const now = Date.now();
    assert.deepEqual(
      desktopState('b', { mode: 'paused', holder: desktopHolder('b'), takeover: true, expiresAt: now + 1000 }),
      {
        mode: 'paused',
        mine: true,
        expiresAt: now + 1000,
        taking: true,
        revision: 0,
      },
    );
    assert.deepEqual(desktopState('b', { mode: 'agent', revision: 3 }), {
      mode: 'agent',
      mine: false,
      expiresAt: null,
      taking: false,
      revision: 3,
    });
  });
});

describe('desktopControl', () => {
  it('reads the current state', async () => {
    assert.equal((await desktopControl('key-a', id, 'get')).mode, 'agent');
  });

  it('refuses an unknown action', async () => {
    await assert.rejects(desktopControl('key-a', id, 'release'), /Invalid control action/);
  });

  it('requests and renews through the takeover actions', async () => {
    assert.equal((await desktopControl('key-a', id, 'request')).taking, true);
    await desktopControl('key-a', id, 'acquire');
    assert.equal((await desktopControl('key-a', id, 'renew')).mine, true);
  });

  it('acquires control for the desktop', async () => {
    const state = await desktopControl('key-a', id, 'acquire');
    assert.deepEqual([state.mode, state.mine], ['human', true]);
  });

  it('returns control to the agent', async () => {
    await desktopControl('key-a', id, 'acquire');
    assert.equal((await desktopControl('key-a', id, 'return')).mode, 'agent');
  });

  it('resumes automation left paused with no holder', async () => {
    await control().takeover('key-a', id, 'acquire', 'someone');
    await control().takeover('key-a', id, 'release', 'someone');
    assert.equal((await desktopControl('key-a', id, 'return')).mode, 'agent');
  });

  it('fails when the browser disconnects during the takeover', async () => {
    await assert.rejects(
      desktopControl('key-a', id, 'acquire', () => false),
      /disconnected during takeover/,
    );
  });

  it('waits for the agent’s command, then gives up after ten seconds leaving automation paused', async () => {
    mock.timers.enable({ apis: ['Date', 'setTimeout'] });
    registry.get(id).pending = 1;
    const attempt = desktopControl('key-a', id, 'acquire');
    const outcome = attempt.catch((e) => e);
    await advance(100, 101);
    assert.match((await outcome).message, /still running/);
    assert.equal((await desktopControl('key-a', id, 'get')).mode, 'paused');
  });
});
