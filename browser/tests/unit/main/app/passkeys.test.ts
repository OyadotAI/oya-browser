/** Native passkey choices fail closed, preserve the source document, and never silently pick an account. */
import { it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { Passkeys, configurePasskeys } from '../../../../src/main/app/passkeys.ts';
import { PASSKEY_PROMPT_TIMEOUT_MS } from '../../../../src/main/app/constants.ts';

/** Minimal native seams; no credentials or hardware are used. */
function fixture() {
  const contents = Object.assign(new EventEmitter(), { isDestroyed: () => false });
  const details = {
    frame: { detached: false },
    relyingPartyId: 'example.com',
    accounts: [
      { credentialId: 'one', name: 'Person' },
      { credentialId: 'two', name: 'Other' },
    ],
  };
  const boxes: any[] = [];
  const deps: any = {
    governance: { configuration: null },
    control: { snapshot: () => ({ interactive: true }) },
    tabs: { getActiveView: () => ({ webContents: contents }) },
    electron: {
      webContents: { fromFrame: () => contents },
      BrowserWindow: { fromWebContents: () => null },
      dialog: {
        showMessageBox: async (options: any) => {
          boxes.push(options);
          return { response: 2 };
        },
      },
    },
  };
  return { contents, details, boxes, deps, service: new Passkeys(deps) };
}

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
      f.deps.tabs.getActiveView = () => null;
    },
    (f: any) => {
      f.deps.control.snapshot = () => ({ interactive: false });
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
  f.contents.emit('did-start-navigation');
  pending.resolve({ response: 1 });
  assert.equal(await result, undefined);
  assert.equal(f.contents.listenerCount('destroyed'), 0);
});
it('rechecks human control when the prompt resolves', async () => {
  const f = fixture();
  f.deps.electron.dialog.showMessageBox = async () => {
    f.deps.control.snapshot = () => ({ interactive: false });
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
    f.deps.electron.dialog.showMessageBox = (options: any) =>
      new Promise((resolve) => options.signal.addEventListener('abort', () => resolve({ response: 0 })));
    const result = f.service.choose(f.details as any);
    mock.timers.tick(PASSKEY_PROMPT_TIMEOUT_MS);
    assert.equal(await result, undefined);
  } finally {
    mock.timers.reset();
  }
});
