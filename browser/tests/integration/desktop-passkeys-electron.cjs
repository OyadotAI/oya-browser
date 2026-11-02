/** Actual Oya frame/window events with a controlled account chooser; no biometric or credential claim. */
const assert = require('node:assert/strict');
const { setTimeout: pause } = require('node:timers/promises');
const electron = require('electron');
const { app, BrowserWindow } = electron;
const { Passkeys } = require('../../src/main/app/passkeys.ts');
const { AgentTab, inLane } = require('../../src/main/control/agent-tab.ts');
const { WindowTabEvents } = require('../../src/main/windows/tab-events.ts');
if (!process.env.OYA_PASSKEY_PROFILE) throw Error('Run desktop-passkeys.mjs');
app.setPath('userData', process.env.OYA_PASSKEY_PROFILE);
app.on('window-all-closed', () => {});
app.on('web-contents-created', (_event, contents) =>
  Object.defineProperty(contents, 'debugger', {
    get() {
      throw Error('Passkey fixture accessed a debugger');
    },
  }),
);
const deadline = setTimeout(() => {
  console.error('Passkey fixture timed out');
  app.exit(1);
}, 30000);

/** Wait only for a bounded native frame/document observation. */
async function until(read) {
  for (let attempt = 0; attempt < 100; attempt++) {
    if (await read()) return;
    await pause(20);
  }
  throw Error('Native frame observation timed out');
}

/** Each window owns its own selected tab; agent routing intentionally names the other window. */
async function owner(id) {
  const window = new BrowserWindow({ show: true, webPreferences: { sandbox: true, contextIsolation: true } });
  await window.loadURL(
    'data:text/html,<h1>Owned passkey fixture</h1><iframe id="requester" name="requester" srcdoc="<p>Requesting frame</p>"></iframe>',
  );
  const tab = { id, view: { webContents: window.webContents } };
  return {
    shell: { window },
    tabs: { activeTabId: id, list: [tab], find: (target) => (target === id ? tab : undefined) },
  };
}

/** Production eligibility and actual native frame events are tested; the OS dialog seam returns fixture account ids. */
async function main() {
  await app.whenReady();
  const first = await owner(1),
    second = await owner(2),
    scopes = [first, second];
  const state = { connected: true, mode: 'agent', localClients: 0, interactive: false };
  const deps = {
    tabs: first.tabs,
    control: { snapshot: () => state },
    governance: { configuration: null },
    persona: { policyEpoch: Symbol(), partitionName: () => 'persist:test' },
    windows: {
      owner: (id) => scopes.find((scope) => scope.tabs.list.some((tab) => tab.id === id)),
      ownerOfContents: (contents) =>
        scopes.find((scope) => scope.tabs.list.some((tab) => tab.view.webContents === contents)),
      allTabs: () => scopes.flatMap((scope) => scope.tabs.list),
      tabEvents: new WindowTabEvents(),
    },
    electron: { ...electron, dialog: { showMessageBox: async () => ({ response: 1 }) } },
  };
  const service = new Passkeys(deps);
  deps.agentTab = new AgentTab(deps, () => service.sync());
  deps.agentTab.claim(1);
  const contents = second.shell.window.webContents;
  const details = () => ({
    frame: contents.mainFrame,
    relyingPartyId: 'fixture.test',
    accounts: [{ name: 'Fixture account', credentialId: 'fixture-only-id' }],
  });
  deps.electron.dialog.showMessageBox = async (parent) => {
    assert.equal(parent, second.shell.window);
    return { response: 1 };
  };
  assert.equal(await service.choose(details()), 'fixture-only-id');
  assert.equal(await service.choose({ ...details(), frame: first.shell.window.webContents.mainFrame }), undefined);
  deps.electron.dialog.showMessageBox = async (_parent, options) => {
    await contents.executeJavaScript('location.hash="same-document"');
    await contents.executeJavaScript(
      'new Promise(resolve=>{const f=document.createElement("iframe");f.onload=resolve;f.srcdoc="<p>Unrelated frame</p>";document.body.append(f);})',
    );
    assert.equal(options.signal.aborted, false);
    return { response: 1 };
  };
  assert.equal(await service.choose(details()), 'fixture-only-id');
  const child = contents.mainFrame.frames.find((frame) => frame.name === 'requester' && frame.url === 'about:srcdoc');
  assert.ok(child);
  deps.electron.dialog.showMessageBox = async (_parent, options) => {
    await contents.executeJavaScript('document.getElementById("requester").srcdoc="<p>Replacement</p>"');
    await until(() => options.signal.aborted);
    return { response: 1 };
  };
  assert.equal(await service.choose({ ...details(), frame: child }), undefined);
  await popupChecks(deps, service, second);
  deps.electron.dialog.showMessageBox = async (_parent, options) => {
    deps.agentTab.claim(2);
    assert.equal(options.signal.aborted, true);
    return { response: 1 };
  };
  assert.equal(await service.choose(details()), undefined);
  assert.equal(contents.listenerCount('did-start-navigation'), 0);
  console.log(
    'PASS Oya passkey ownership: scoped agent, correct window, same-document/unrelated frame preservation, requesting-frame replacement and takeover cancellation; no Touch ID ceremony tested',
  );
  for (const scope of scopes) scope.shell.window.destroy();
  clearTimeout(deadline);
  app.exit(0);
}

/** Real linked popup focus does not replace the human-ownership requirement for its native opener. */
async function popupChecks(deps, service, scope) {
  const contents = scope.shell.window.webContents;
  contents.setWindowOpenHandler(() => ({ action: 'allow' }));
  const created = new Promise((resolve) => contents.once('did-create-window', resolve));
  await contents.executeJavaScript('void window.open("about:blank", "passkey-fixture")', true);
  const popup = await created;
  const tab = { id: 3, openerId: 2, window: popup, view: { webContents: popup.webContents } };
  scope.tabs.list.push(tab);
  const previousFind = scope.tabs.find;
  scope.tabs.find = (id) => scope.tabs.list.find((item) => item.id === id);
  popup.show();
  popup.focus();
  await until(() => popup.isFocused());
  const details = {
    frame: popup.webContents.mainFrame,
    relyingPartyId: 'fixture.test',
    accounts: [{ name: 'Fixture popup', credentialId: 'popup-only-id' }],
  };
  let prompts = 0;
  deps.electron.dialog.showMessageBox = async (parent) => {
    assert.equal(parent, popup);
    prompts++;
    return { response: 1 };
  };
  assert.equal(await service.choose(details), 'popup-only-id', 'unrelated agent does not block a human popup');
  await nativeSheetChecks(deps, service, popup, details);
  inLane('popup-test', () => deps.agentTab.claim(2));
  assert.equal(await service.choose(details), undefined, 'claimed opener refuses unclaimed popup');
  assert.equal(prompts, 1);
  inLane('popup-test', () => deps.agentTab.release());
  deps.electron.dialog.showMessageBox = async (_parent, options) => {
    inLane('popup-test', () => deps.agentTab.claim(2));
    inLane('popup-test', () => deps.agentTab.release());
    assert.equal(options.signal.aborted, true);
    return { response: 1 };
  };
  assert.equal(await service.choose(details), undefined, 'late choice cannot outlive opener takeover');
  scope.tabs.list.pop();
  scope.tabs.find = previousFind;
  popup.destroy();
}

/** A real macOS sheet borrows parent focus without revoking the request that owns the native dialog. */
async function nativeSheetChecks(deps, service, popup, details) {
  if (process.platform !== 'darwin') return;
  const previous = deps.electron.dialog.showMessageBox;
  const observed = { timed: false, unfocused: 0, revokedBeforeCancel: false };
  deps.electron.dialog.showMessageBox = async (parent, options) => {
    const stop = new AbortController();
    const poll = setInterval(() => {
      if (!popup.isFocused()) observed.unfocused++;
    }, 25);
    const timer = setTimeout(() => {
      observed.timed = true;
      observed.revokedBeforeCancel = options.signal.aborted;
      stop.abort();
    }, 750);
    try {
      return await electron.dialog.showMessageBox(parent, {
        ...options,
        signal: AbortSignal.any([options.signal, stop.signal]),
      });
    } finally {
      clearInterval(poll);
      clearTimeout(timer);
    }
  };
  try {
    popup.focus();
    await until(() => popup.isFocused());
    assert.equal(await service.choose(details), undefined, 'fixture deliberately cancels the native sheet');
    assert.equal(observed.timed, true, 'production must not self-cancel before the controlled native dismissal');
    assert.equal(observed.revokedBeforeCancel, false, 'native sheet focus is not a takeover');
    assert.ok(observed.unfocused > 0, 'real macOS sheet reproduced temporary parent focus loss');
    console.log('PASS native macOS passkey sheet retains its popup request until deliberate cancellation');
  } finally {
    deps.electron.dialog.showMessageBox = previous;
  }
  popup.focus();
  await until(() => popup.isFocused());
}
main().catch((error) => {
  console.error(error);
  clearTimeout(deadline);
  app.exit(1);
});
