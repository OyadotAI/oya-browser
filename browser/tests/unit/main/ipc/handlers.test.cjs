/**
 * Unit tests for the IPC handlers: the human-control guard on page actions,
 * settings, preferences, exports, chat, and the workspace commands.
 */
const { describe, it, beforeEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { registerIpc } = require('../../../../main/ipc/index.cjs');
const { mainCtx } = require('../../support/main-ctx.cjs');

describe('IPC handlers', () => {
  let ctx, call;
  beforeEach(() => {
    ctx = mainCtx();
    const handlers = new Map();
    registerIpc((channel, fn) => handlers.set(channel, fn), ctx);
    call = (channel, ...args) => handlers.get(channel)({}, ...args);
  });

  it('refuses page actions while an agent has control', async () => {
    ctx.shield.requireHumanControl = () => {
      throw new Error('Take control');
    };
    for (const channel of ['navigate', 'go-back', 'go-forward', 'new-tab', 'close-tab', 'start-recording']) {
      await assert.rejects(async () => call(channel, 'x'), /Take control/, channel);
    }
  });

  it('enters browsing mode on the first navigation', async () => {
    let entered;
    ctx.shell.browsingMode = false;
    ctx.tabs = { enterBrowsingMode: (url) => (entered = url) };
    assert.equal(await call('navigate', 'a.test'), undefined);
    assert.equal(entered, 'a.test');
  });

  it('opens a new tab on the home page and records it', () => {
    const recorded = [];
    ctx.tabs = { createTab: () => 5 };
    ctx.recorder.recordNavigation = (url) => recorded.push(url);
    assert.equal(call('new-tab'), 5);
    assert.deepEqual(recorded, ['https://google.com']);
  });

  it('saves settings and reconnects with them', () => {
    assert.equal(call('save-config', { apiKey: 'k2' }), true);
    assert.equal(ctx.config.values.apiKey, 'k2');
    assert.deepEqual([ctx.config.saves, ctx.socket.disconnects, ctx.socket.connects], [1, 1, 1]);
  });

  it('answers a failed control change with the state as it stands', async () => {
    ctx.control.change = async () => {
      throw new Error('Another operator has control');
    };
    const answer = await call('change-control', 'acquire');
    assert.equal(answer.error, 'Another operator has control');
    assert.equal(answer.state.mode, 'human');
  });

  it('keeps only known themes and panes', () => {
    assert.equal(call('save-ui-preferences', null), false);
    call('save-ui-preferences', { theme: 'dark', pane: 'evil' });
    assert.deepEqual(ctx.config.values.ui, { theme: 'dark' });
    assert.equal(call('get-ui-preferences').pane, 'chat');
  });

  it('refuses to save a profile while offline', async () => {
    ctx.socket.ready = false;
    await assert.rejects(call('save-profile'), /Connect the desktop/);
  });

  it('flushes cookies and storage when saving a profile', async () => {
    const steps = [];
    ctx.cookies = { flushCookieChanges: () => steps.push('flush'), dumpCookies: async () => steps.push('dump') };
    ctx.persona.session = () => ({
      cookies: { flushStore: async () => steps.push('store') },
      flushStorageData: () => steps.push('storage'),
    });
    await call('save-profile');
    assert.deepEqual(steps, ['flush', 'dump', 'store', 'storage']);
    assert.deepEqual(ctx.socket.sent.at(-1), { type: 'profile_flush' });
  });

  it('exports Playwright code owner-only, under a safe name', async () => {
    const file = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'oya-export-')), 'x.mjs');
    ctx.electron.dialog.answers.saveDialog.push({ canceled: false, filePath: file });
    assert.deepEqual(await call('export-playwright', { code: 'test()', name: '../evil' }), { saved: true });
    assert.equal(ctx.electron.dialog.asked[0][1].defaultPath, 'playbook.mjs');
    assert.equal(fs.statSync(file).mode & 0o777, 0o600);
    await assert.rejects(call('export-playwright', { code: 1 }), /Invalid Playwright export/);
    assert.deepEqual(await call('export-playwright', { code: '' }), { canceled: true });
  });

  it('asks before discarding a recording', async () => {
    ctx.electron.dialog.answers.messageBox.push({ response: 1 });
    assert.equal(await call('confirm-discard-recording'), true);
    assert.equal(await call('confirm-discard-recording'), false);
  });

  it("saves the agent's last run as a playbook by name alone", async () => {
    ctx.config.values = { serverUrl: 'ws://s.test/ws', apiKey: 'k' };
    const answer = { name: 'lookup', steps: 3 };
    const fetch = mock.method(globalThis, 'fetch', async () => ({
      status: 200,
      text: async () => JSON.stringify(answer),
    }));
    assert.deepEqual(await call('save-chat-playbook', 'lookup'), answer);
    const [url, init] = fetch.mock.calls[0].arguments;
    assert.equal(url, 'http://s.test/api/browsers/b1/playbooks');
    assert.deepEqual(JSON.parse(init.body), { name: 'lookup' });
    fetch.mock.restore();
  });

  it('refuses to save a playbook while disconnected', async () => {
    ctx.socket.ready = false;
    assert.deepEqual(await call('save-chat-playbook', 'lookup'), { error: 'Not connected to server' });
  });

  it('relays chat to the server and explains a non-JSON answer', async () => {
    ctx.config.values = { serverUrl: 'ws://s.test/ws', apiKey: 'k' };
    const fetch = mock.method(globalThis, 'fetch', async () => ({ status: 200, text: async () => '{"reply":"hi"}' }));
    assert.deepEqual(await call('send-chat', [{ role: 'user' }]), { reply: 'hi' });
    assert.equal(fetch.mock.calls[0].arguments[0], 'http://s.test/api/browsers/b1/chat');
    fetch.mock.mockImplementation(async () => ({ status: 502, text: async () => '<html>' }));
    assert.deepEqual(await call('send-chat', []), { error: 'Server returned 502: <html>' });
    ctx.socket.ready = false;
    assert.deepEqual(await call('send-chat', []), { error: 'Not connected to server' });
    fetch.mock.restore();
  });

  it('toggles and resizes the dev panel through the layout', () => {
    ctx.layout = { toggle: (reduced) => `toggled ${reduced}`, resize: (w) => w * 2 };
    assert.equal(call('toggle-dev-panel', true), 'toggled true');
    assert.equal(call('resize-dev-panel', 3), 6);
  });
});

describe('workspace channel', () => {
  let ctx, call;
  beforeEach(() => {
    ctx = mainCtx();
    const handlers = new Map();
    registerIpc((channel, fn) => handlers.set(channel, fn), ctx);
    call = (...args) => handlers.get('workspace')({}, ...args);
    ctx.workspace = {
      draft: { id: 'd', steps: [{ action: 'click' }], secrets: ['pw'] },
      busy: () => false,
      snapshot: () => 'snapshot',
      edit(command) {
        this.edited = command;
        return 'edited';
      },
      start: async (command) => ['started', command.type],
      control: (command) => ['control', command],
    };
    ctx.recorder = {
      recording: false,
      adopt(steps, secrets) {
        this.adopted = [steps, secrets];
      },
      queueRecording: (work) => work(),
      startRecording: async (resume) => (ctx.resumed = resume),
    };
  });

  it('waits for the workspace to exist', async () => {
    ctx.workspace = null;
    await assert.rejects(call({ type: 'get' }), /Workspace is starting/);
  });

  it('treats unknown command types as edits, and the recording follows', async () => {
    assert.equal(await call({ type: 'rename', name: 'x' }), 'edited');
    assert.deepEqual(ctx.recorder.adopted, [[{ action: 'click' }], ['pw']]);
    assert.notEqual(ctx.recorder.adopted[0], ctx.workspace.draft.steps, 'the recording gets its own copy');
  });

  it('runs a validation only once the person confirms', async () => {
    assert.equal(await call({ type: 'validate' }), 'snapshot');
    ctx.electron.dialog.answers.messageBox.push({ response: 1 });
    assert.deepEqual(await call({ type: 'validate' }), ['started', 'validate']);
  });

  it('hands control back before resuming a paused run', async () => {
    assert.deepEqual(await call({ type: 'control', command: 'resume' }), ['control', 'resume']);
    assert.deepEqual(ctx.control.changes, ['return']);
    await call({ type: 'control', command: 'stop' });
    assert.deepEqual(ctx.control.changes, ['return']);
  });

  it('resumes a paused recording', async () => {
    assert.equal(await call({ type: 'resume-recording' }), 'snapshot');
    assert.equal(ctx.resumed, true);
  });

  it('refuses to pick a target during recording or without a page', async () => {
    ctx.recorder.recording = true;
    await assert.rejects(call({ type: 'pick' }), /Pause recording/);
    ctx.recorder.recording = false;
    ctx.tabs = { getActiveView: () => null };
    await assert.rejects(call({ type: 'pick' }), /Open a page first/);
  });

  it('saves a diagnostics report owner-only where the person chooses', async () => {
    const file = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'oya-support-')), 'd.json');
    ctx.workspace.support = () => ({ ok: 1 });
    ctx.electron.dialog.answers.saveDialog.push({ canceled: false, filePath: file });
    assert.equal(await call({ type: 'support' }), 'snapshot');
    assert.deepEqual(JSON.parse(fs.readFileSync(file, 'utf8')), { ok: 1 });
    assert.equal(fs.statSync(file).mode & 0o777, 0o600);
  });
});
