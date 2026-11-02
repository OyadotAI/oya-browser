/** Native passkey choices fail closed, preserve the source document, and never silently pick an account. */
import { it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { configurePasskeys } from '../../../../src/main/app/passkeys.ts';
import { PASSKEY_PROMPT_TIMEOUT_MS } from '../../../../src/main/app/constants.ts';

import { fixture } from '../../support/passkey-fixture.ts';

it('permits the selected human tab while an agent owns another tab and parents to its real window', async () => {
  const f = fixture();
  const checked: number[] = [];
  f.deps.agentTab.humanMay = (id: number) => (checked.push(id), id === f.tab.id);
  f.deps.control = { snapshot: () => ({ interactive: false }) };
  f.deps.tabs = { activeTabId: 99, getActiveView: () => null };
  assert.equal(await f.service.choose(f.details as any), 'two');
  assert.ok(checked.every((id) => id === f.tab.id));
  assert.deepEqual(f.parents, [f.window]);
});

it('same-document navigation and unrelated frame loads leave the requesting chooser active', async () => {
  for (const event of [
    { isMainFrame: true, isSameDocument: true },
    { isMainFrame: false, isSameDocument: false, frame: {} },
  ]) {
    const f = fixture();
    f.deps.electron.dialog.showMessageBox = async (_parent, options) => {
      f.contents.emit('did-start-navigation', event);
      assert.equal(options.signal.aborted, false);
      return { response: 1 };
    };
    assert.equal(await f.service.choose(f.details as any), 'one');
  }
});

it('requester and ancestor replacement cancel without confusing an unrelated iframe', async () => {
  for (const target of ['requester', 'ancestor']) {
    const f = fixture();
    const parent = { parent: null };
    f.details.frame.parent = parent;
    f.deps.electron.dialog.showMessageBox = async (_parent, options) => {
      f.contents.emit('did-start-navigation', {
        isMainFrame: false,
        isSameDocument: false,
        frame: target === 'requester' ? f.details.frame : parent,
      });
      assert.equal(options.signal.aborted, true);
      return { response: 1 };
    };
    assert.equal(await f.service.choose(f.details as any), undefined);
  }
});

it('control revocation cannot be undone by returning control before a late account response', async () => {
  const f = fixture();
  f.deps.electron.dialog.showMessageBox = async (_parent, options) => {
    f.deps.agentTab.humanMay = () => false;
    f.service.sync();
    f.deps.agentTab.humanMay = () => true;
    assert.equal(options.signal.aborted, true);
    return { response: 1 };
  };
  assert.equal(await f.service.choose(f.details as any), undefined);
});

it('profile epoch, native session, tab ownership and window lifetime are rechecked after account selection', async () => {
  for (const revoke of [
    (f) => (f.deps.persona.policyEpoch = Symbol()),
    (f) => (f.deps.persona.partitionName = () => 'persist:two'),
    (f) => (f.contents.session = {}),
    (f) => (f.deps.windows.ownerOfContents = () => undefined),
    (f) => (f.owner.tabs.activeTabId = 9),
    (f) => f.window.emit('closed'),
  ]) {
    const f = fixture();
    f.deps.electron.dialog.showMessageBox = async () => {
      revoke(f);
      return { response: 1 };
    };
    assert.equal(await f.service.choose(f.details as any), undefined);
    assert.equal(f.window.listenerCount('closed'), 0);
  }
});

it('only a tracked focused human popup may open the native account chooser', async () => {
  const f = fixture();
  f.tab.window = f.window;
  f.owner.tabs.activeTabId = 99;
  assert.equal(await f.service.choose(f.details as any), 'two');
  f.window.isFocused = () => false;
  assert.equal(await f.service.choose(f.details as any), undefined);
});

it('configures only macOS with a valid signing-bound access group', () => {
  const calls: any[] = [];
  const app = { configureWebAuthn: (options: any) => calls.push(options) };
  for (const group of ['', 'bad', 'AAAAAAAAAA.other']) configurePasskeys(app, group, 'darwin');
  configurePasskeys(app, 'AAAAAAAAAA.ai.oya.browser.webauthn', 'win32');
  assert.equal(calls.length, 0);
  configurePasskeys(app, 'AAAAAAAAAA.ai.oya.browser.webauthn', 'darwin');
  assert.deepEqual(calls, [{ touchID: { keychainAccessGroup: 'AAAAAAAAAA.ai.oya.browser.webauthn' } }]);
});
it('returns only the explicitly chosen account and defaults to Cancel', async () => {
  const f = fixture();
  assert.equal(await f.service.choose(f.details as any), 'two');
  assert.deepEqual(f.boxes[0].buttons, ['Cancel', 'Person', 'Other']);
  assert.equal(f.boxes[0].defaultId, 0);
  assert.equal(f.contents.listenerCount('did-start-navigation'), 0);
});
it('cancels rather than choosing an account on cancel or an invalid response', async () => {
  for (const response of [0, -1, 99]) {
    const f = fixture();
    f.deps.electron.dialog.showMessageBox = async () => ({ response });
    assert.equal(await f.service.choose(f.details as any), undefined);
  }
});
it('refuses background, remote-controlled, governed and detached requesters', async () => {
  const edits = [
    (f: any) => {
      f.owner.tabs.activeTabId = 9;
    },
    (f: any) => {
      f.deps.agentTab.humanMay = () => false;
    },
    (f: any) => {
      f.deps.governance.configuration = {};
    },
    (f: any) => {
      f.details.frame.detached = true;
    },
  ];
  for (const edit of edits) {
    const f = fixture();
    edit(f);
    assert.equal(await f.service.choose(f.details as any), undefined);
    assert.equal(f.boxes.length, 0);
  }
});
it('fences navigation and concurrent prompts and cleans up listeners', async () => {
  const f = fixture();
  const pending = Promise.withResolvers<any>();
  f.deps.electron.dialog.showMessageBox = () => pending.promise;
  const result = f.service.choose(f.details as any);
  assert.equal(await f.service.choose(f.details as any), undefined);
  f.contents.emit('did-start-navigation', { isMainFrame: true, isSameDocument: false, frame: f.details.frame });
  pending.resolve({ response: 1 });
  assert.equal(await result, undefined);
  assert.equal(f.contents.listenerCount('destroyed'), 0);
});
it('rechecks human control when the prompt resolves', async () => {
  const f = fixture();
  f.deps.electron.dialog.showMessageBox = async () => {
    f.deps.agentTab.humanMay = () => false;
    return { response: 1 };
  };
  assert.equal(await f.service.choose(f.details as any), undefined);
});
it('settles native callbacks once on errors and installs only one listener', async () => {
  const f = fixture();
  const session = new EventEmitter();
  const replies: any[] = [];
  f.service.install(session as any);
  f.service.install(session as any);
  f.deps.electron.dialog.showMessageBox = async () => {
    throw Error('dialog unavailable');
  };
  session.emit('select-webauthn-account', {}, f.details, (id: any) => replies.push(id));
  await new Promise((resolve) => setImmediate(resolve));
  assert.deepEqual(replies, [undefined]);
  assert.equal(session.listenerCount('select-webauthn-account'), 1);
  assert.equal(f.contents.listenerCount('destroyed'), 0);
});
it('aborts an abandoned native prompt at the bounded timeout', async () => {
  mock.timers.enable({ apis: ['setTimeout'] });
  try {
    const f = fixture();
    f.deps.electron.dialog.showMessageBox = (_parent: any, options: any) =>
      new Promise((resolve) => options.signal.addEventListener('abort', () => resolve({ response: 0 })));
    const result = f.service.choose(f.details as any);
    mock.timers.tick(PASSKEY_PROMPT_TIMEOUT_MS);
    assert.equal(await result, undefined);
  } finally {
    mock.timers.reset();
  }
});
