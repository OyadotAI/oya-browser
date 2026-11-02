/** A pending native chooser is revoked on exact document/lifetime changes, with bounded observer cleanup. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { fixture } from '../../support/passkey-fixture.ts';
import { PASSKEY_RECHECK_MS } from '../../../../src/main/app/constants.ts';

test('detaching a requesting frame cancels the native dialog before any account choice', async (t) => {
  t.mock.timers.enable({ apis: ['setInterval', 'setTimeout'] });
  const f = fixture();
  f.deps.electron.dialog.showMessageBox = (_window, options) =>
    new Promise((resolve) => options.signal.addEventListener('abort', () => resolve({ response: 1 })));
  const result = f.service.choose(f.details as any);
  f.details.frame.detached = true;
  t.mock.timers.tick(PASSKEY_RECHECK_MS);
  assert.equal(await result, undefined);
  assert.equal(f.contents.listenerCount('did-start-navigation'), 0);
  assert.equal(f.window.listenerCount('closed'), 0);
});

test('a tab replacing its native view cannot inherit a chooser even with the same profile session', async () => {
  const f = fixture();
  f.deps.electron.dialog.showMessageBox = async () => {
    f.tab.view = { webContents: { ...f.contents, session: f.contents.session } };
    return { response: 1 };
  };
  assert.equal(await f.service.choose(f.details as any), undefined);
  assert.equal(f.contents.listenerCount('did-start-navigation'), 0);
});

test('selecting another tab revokes immediately and returning cannot revive the old prompt', async () => {
  const f = fixture();
  f.deps.electron.dialog.showMessageBox = async (_window, options) => {
    f.owner.tabs.activeTabId = 99;
    f.deps.windows.tabEvents.changed();
    f.owner.tabs.activeTabId = f.tab.id;
    assert.equal(options.signal.aborted, true);
    return { response: 1 };
  };
  assert.equal(await f.service.choose(f.details as any), undefined);
});

test('popup selection refuses an agent-owned opener even when the focused popup has no agent claim', async () => {
  const f = fixture();
  f.tab.window = f.window;
  f.tab.openerId = 1;
  f.owner.tabs.list.push({ id: 1 });
  f.deps.agentTab.humanMay = (id) => id !== 1;
  assert.equal(await f.service.choose(f.details as any), undefined);
  assert.deepEqual(f.boxes, []);
});

test('a nested popup cannot bypass its agent-owned grandparent', async () => {
  const f = fixture();
  f.tab.window = f.window;
  f.tab.openerId = 3;
  f.owner.tabs.list.push({ id: 3, openerId: 1 }, { id: 1 });
  f.deps.agentTab.humanMay = (id) => id !== 1;
  assert.equal(await f.service.choose(f.details as any), undefined);
  assert.deepEqual(f.boxes, []);
});

test('a missing or cyclic popup opener cannot be treated as a fresh human-owned source', async () => {
  for (const opener of [undefined, 9, 2]) {
    const f = fixture();
    f.tab.window = f.window;
    f.tab.openerId = opener;
    assert.equal(await f.service.choose(f.details as any), undefined);
    assert.deepEqual(f.boxes, []);
  }
});

test('claiming or replacing an opener while its popup chooser is pending revokes the late account result', async () => {
  for (const revoke of [
    (f) => {
      f.deps.agentTab.humanMay = (id) => id !== 1;
      f.service.sync();
      f.deps.agentTab.humanMay = () => true;
    },
    (f) => {
      f.owner.tabs.list[1] = { id: 1 };
    },
    (f) => {
      f.tab.openerId = 9;
    },
    (f) => {
      f.owner.tabs.list.pop();
    },
    (f) => {
      const moved = { tabs: { find: () => f.owner.tabs.list[1] } };
      f.deps.windows.owner = (id) => (id === 1 ? moved : f.owner);
    },
  ]) {
    const f = fixture();
    f.tab.window = f.window;
    f.tab.openerId = 1;
    f.owner.tabs.list.push({ id: 1 });
    f.deps.electron.dialog.showMessageBox = async () => {
      revoke(f);
      return { response: 1 };
    };
    assert.equal(await f.service.choose(f.details as any), undefined);
  }
});
