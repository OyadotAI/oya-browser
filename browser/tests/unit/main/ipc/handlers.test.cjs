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

  it('reports browsing mode in the status, so a late renderer leaves the setup screen', async () => {
    ctx.tabs = { getActiveView: () => null };
    ctx.shell.browsingMode = true;
    assert.equal((await call('get-status')).browsing, true);
    ctx.shell.browsingMode = false;
    assert.equal((await call('get-status')).browsing, false);
  });

  it('opens only the console of a ws or wss server, never another scheme', async () => {
    assert.equal(
      await call('open-console', 'wss://oyabrowser.com/ws'),
      'https://oyabrowser.com/dashboard?connect=desktop',
    );
    assert.equal(await call('open-console', 'file:///etc/passwd'), null);
    assert.deepEqual(ctx.electron.shell.opened, ['https://oyabrowser.com/dashboard?connect=desktop']);
  });

  it('logs out by forgetting the key and returning to the welcome screen, keeping the server and name', async () => {
    let left = false;
    ctx.tabs = { leaveBrowsingMode: () => (left = true) };
    ctx.config.values = { serverUrl: 'wss://s/ws', apiKey: 'k', browserName: 'Desk' };
    ctx.socket.browserId = 'b1';
    await call('sign-out');
    assert.deepEqual(ctx.config.values, {
      serverUrl: 'wss://s/ws',
      apiKey: '',
      browserName: 'Desk',
      signedOut: true,
      keyFromApp: false,
    });
    assert.equal(ctx.socket.browserId, null);
    assert.ok(left);
  });

  it('signing in again clears the logged-out mark', async () => {
    ctx.config.values = { signedOut: true };
    await call('save-config', { serverUrl: 'wss://s/ws', apiKey: 'k2' });
    assert.equal(ctx.config.values.signedOut, false);
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

  it('records Back and Forward before going, and records nothing when there is nowhere to go', async () => {
    const order = [];
    const history = { canGoBack: () => true, canGoForward: () => false };
    const contents = {
      navigationHistory: history,
      goBack: () => order.push('back'),
      goForward: () => order.push('fwd'),
    };
    ctx.tabs = { getActiveView: () => ({ webContents: contents }) };
    ctx.recorder.recordHistory = async (action) => order.push(action);
    await call('go-back');
    await call('go-forward');
    assert.deepEqual(order, ['go_back', 'back']);
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

  it('keeps a key entered in the app over OYA_API_KEY on later launches', () => {
    call('save-config', { apiKey: 'k2' });
    assert.equal(ctx.config.values.keyFromApp, true);
    call('save-config', { persona: 'p-work' });
    assert.equal(ctx.config.values.keyFromApp, true, 'choosing a profile keeps it');
  });

  it("starts a fresh session and the default persona on another project's key or server", () => {
    ctx.config.values = { serverUrl: 'wss://s/ws', apiKey: 'k1', persona: 'm-old-project' };
    ctx.socket.browserId = 'b-old';
    call('save-config', { serverUrl: 'wss://s/ws', apiKey: 'k2' });
    assert.deepEqual([ctx.socket.browserId, ctx.config.values.persona], [null, 'default']);
  });

  it('keeps the session and persona when the key and server stay the same', () => {
    ctx.config.values = { serverUrl: 'wss://s/ws', apiKey: 'k1', persona: 'p-work' };
    ctx.socket.browserId = 'b1';
    call('save-config', { serverUrl: 'wss://s/ws', apiKey: 'k1', browserName: 'Desk' });
    assert.deepEqual([ctx.socket.browserId, ctx.config.values.persona], ['b1', 'p-work']);
  });

  it('answers a failed control change with the state as it stands', async () => {
    ctx.control.change = async () => {
      throw new Error('Another operator has control');
    };
    const answer = await call('change-control', 'acquire');
    assert.equal(answer.error, 'Another operator has control');
    assert.equal(answer.state.mode, 'human');
  });

  it('keeps only known themes, and no pane: launch always opens on Ask', () => {
    assert.equal(call('save-ui-preferences', null), false);
    call('save-ui-preferences', { theme: 'dark', pane: 'record', pageFormat: 'xml' });
    assert.deepEqual(ctx.config.values.ui, { theme: 'dark' });
  });

  it('saves the default page format chosen in settings, markdown until one is', () => {
    assert.equal(call('get-ui-preferences').pageFormat, 'markdown');
    assert.deepEqual(call('get-ui-preferences').pageFormats, ['markdown', 'toon', 'jsonl']);
    call('save-ui-preferences', { pageFormat: 'jsonl' });
    assert.equal(call('get-ui-preferences').pageFormat, 'jsonl');
  });

  it('renders a kept analysis again in the format asked for, and nothing for an unknown one', () => {
    const analysis = { facts: { url: 'https://a.test/' }, blocks: [{ region: 'main', kind: 'h1', text: 'Hi' }] };
    assert.equal(
      call('render-page', analysis, 'jsonl'),
      '{"page":{"url":"https://a.test/"}}\n{"region":"main","kind":"h1","text":"Hi"}',
    );
    assert.equal(call('render-page', analysis, 'xml'), '');
    assert.equal(call('render-page', { markdown: '# old' }, 'toon'), '');
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

  /** A server config: the key runs on OpenAI's gpt-4.1 with its own key, and offers two providers. */
  const SERVER = {
    llm_provider: 'openai',
    effective: { hasLlmKey: true, model: 'gpt-4.1', baseUrl: 'https://api.openai.com/v1' },
    llm_catalog: [
      { id: 'openai', base: 'https://api.openai.com/v1', model: 'gpt-4o-mini', models: [] },
      { id: 'openrouter', base: 'https://openrouter.ai/api/v1', model: 'a/b', models: [] },
    ],
  };

  /** Fakes the server: GET /config answers `config`, POST /config answers ok. Returns the fetch mock. */
  const fakeServer = (config = SERVER) => {
    ctx.config.values = { serverUrl: 'ws://s.test/ws', apiKey: 'k' };
    return mock.method(globalThis, 'fetch', async (_url, init) => ({
      ok: true,
      json: async () => (init?.method === 'POST' ? { ok: true } : config),
    }));
  };

  /** The body of the POST /config a save sent. */
  const posted = (fetch) =>
    JSON.parse(fetch.mock.calls.find((c) => c.arguments[1]?.method === 'POST').arguments[1].body);

  it('keeps the model chosen in Ask, and needs no key while the provider stays the same', async () => {
    const fetch = fakeServer();
    assert.deepEqual(await call('save-model-key', { provider: 'openai', model: 'gpt-6-sol', key: '' }), { ok: true });
    assert.deepEqual(posted(fetch), { llm_provider: 'openai', chat_model: 'gpt-6-sol' });
    fetch.mock.restore();
  });

  it('asks for a key when the provider changes, and then resets the endpoint to the new provider', async () => {
    const fetch = fakeServer();
    assert.match((await call('save-model-key', { provider: 'openrouter', model: 'a/b', key: '' })).error, /API key/);
    assert.deepEqual(await call('save-model-key', { provider: 'openrouter', model: 'a/b', key: ' sk-or-1 ' }), {
      ok: true,
    });
    assert.deepEqual(posted(fetch), {
      llm_provider: 'openrouter',
      chat_model: 'a/b',
      openai_api_key: 'sk-or-1',
      openai_base_url: null,
    });
    fetch.mock.restore();
  });

  it('refuses a provider the server does not offer, and quotes the server when it refuses', async () => {
    const fetch = fakeServer();
    assert.match((await call('save-model-key', { provider: 'evil', model: 'x', key: 'k' })).error, /Pick a provider/);
    fetch.mock.mockImplementation(async (_url, init) =>
      init?.method === 'POST'
        ? { ok: false, status: 403, json: async () => ({ error: 'No' }) }
        : { ok: true, json: async () => SERVER },
    );
    assert.deepEqual(await call('save-model-key', { provider: 'openai', model: 'm', key: 'sk-1' }), { error: 'No' });
    fetch.mock.restore();
  });

  it('tells Ask the provider, model and catalog the server runs on, and counts an unreachable server as having one', async () => {
    const fetch = fakeServer();
    assert.deepEqual(await call('model-status'), {
      signedIn: true,
      hasLlmKey: true,
      provider: 'openai',
      model: 'gpt-4.1',
      catalog: SERVER.llm_catalog,
    });
    fetch.mock.mockImplementation(async () => ({ ok: false, status: 500 }));
    assert.deepEqual(await call('model-status'), { signedIn: true, hasLlmKey: true });
    ctx.config.values = { serverUrl: 'ws://s.test/ws', apiKey: '' };
    ctx.socket.ready = false;
    assert.deepEqual(await call('model-status'), { signedIn: false, hasLlmKey: true });
    fetch.mock.restore();
  });

  it('names the provider from the endpoint when the key saved none, and offers the old providers to an older server', async () => {
    const fetch = fakeServer({
      effective: { hasLlmKey: true, model: 'gpt-4o-mini', baseUrl: 'https://api.openai.com/v1' },
    });
    const status = await call('model-status');
    assert.deepEqual(
      status.catalog.map((p) => p.id),
      ['anthropic', 'openai', 'gemini'],
    );
    fetch.mock.restore();
    const fromHost = fakeServer({ ...SERVER, llm_provider: '' });
    assert.equal((await call('model-status')).provider, 'openai');
    fromHost.mock.restore();
  });

  it('sends attached files to the server as the chat data', async () => {
    ctx.config.values = { serverUrl: 'ws://s.test/ws', apiKey: 'k' };
    const fetch = mock.method(globalThis, 'fetch', async () => ({ status: 200, text: async () => '{}' }));
    const data = { file1: { file: 'a.pdf', type: 'application/pdf', b64: 'YQ==' } };
    await call('send-chat', [{ role: 'user', content: 'upload it' }], data);
    assert.deepEqual(JSON.parse(fetch.mock.calls[0].arguments[1].body).data, data);
    fetch.mock.restore();
  });

  it('stops the chat in flight by hanging up, and answers it as stopped', async () => {
    ctx.config.values = { serverUrl: 'ws://s.test/ws', apiKey: 'k' };
    const fetch = mock.method(globalThis, 'fetch', (_url, init) => {
      return new Promise((_resolve, reject) => init.signal.addEventListener('abort', () => reject(init.signal.reason)));
    });
    const asking = call('send-chat', [{ role: 'user' }]);
    await new Promise((resolve) => setImmediate(resolve));
    assert.equal(await call('stop-chat'), true);
    assert.deepEqual(await asking, { error: 'Stopped' });
    assert.equal(await call('stop-chat'), false, 'nothing left to stop');
    fetch.mock.restore();
  });

  it('refuses a second chat while one is running, so two runs never fight over a page', async () => {
    ctx.config.values = { serverUrl: 'ws://s.test/ws', apiKey: 'k' };
    let answer;
    const fetch = mock.method(globalThis, 'fetch', () => new Promise((resolve) => (answer = resolve)));
    const first = call('send-chat', [{ role: 'user' }]);
    assert.match((await call('send-chat', [{ role: 'user' }])).error, /busy/);
    await new Promise((resolve) => setImmediate(resolve));
    answer({ status: 200, text: async () => '{"text":"done"}' });
    assert.deepEqual(await first, { text: 'done' });
    assert.equal(fetch.mock.callCount(), 1);
    fetch.mock.restore();
  });

  it("lists the project's personas for the profile picker, with the one asked for", async () => {
    ctx.config.values = { serverUrl: 'wss://s.test/ws', apiKey: 'k', persona: 'p-2' };
    const personas = [
      { id: 'd', name: 'Default', isDefault: true, fingerprint: {} },
      { id: 'p-2', name: 'Work', isDefault: false },
    ];
    const fetch = mock.method(globalThis, 'fetch', async () => ({ ok: true, json: async () => ({ personas }) }));
    const answer = await call('list-personas');
    assert.equal(fetch.mock.calls[0].arguments[0], 'https://s.test/api/personas');
    assert.equal(fetch.mock.calls[0].arguments[1].headers.Authorization, 'Bearer k');
    assert.deepEqual(answer, {
      personas: [
        { id: 'd', name: 'Default', isDefault: true },
        { id: 'p-2', name: 'Work', isDefault: false },
      ],
      active: 'p-2',
    });
    fetch.mock.restore();
  });

  it('lists no personas while offline, and says why when the server refuses', async () => {
    ctx.socket.ready = false;
    assert.deepEqual(await call('list-personas'), { personas: [], active: 'default' });
    ctx.socket.ready = true;
    const fetch = mock.method(globalThis, 'fetch', async () => ({ ok: false, status: 401 }));
    await assert.rejects(call('list-personas'), /Server returned 401/);
    fetch.mock.restore();
  });

  it('lends control to the agent for an Ask, and gives it back to the person after', async () => {
    ctx.config.values = { serverUrl: 'ws://s.test/ws', apiKey: 'k' };
    const order = [];
    ctx.control.change = async (action) => order.push(action);
    const fetch = mock.method(globalThis, 'fetch', async () => {
      order.push('ask');
      return { status: 200, text: async () => '{"text":"done"}' };
    });
    assert.deepEqual(await call('send-chat', [{ role: 'user' }]), { text: 'done' });
    assert.deepEqual(order, ['return', 'ask', 'acquire']);
    fetch.mock.restore();
  });

  it('leaves control alone when the agent already has it, and resumes paused automation', async () => {
    ctx.config.values = { serverUrl: 'ws://s.test/ws', apiKey: 'k' };
    const fetch = mock.method(globalThis, 'fetch', async () => ({ status: 200, text: async () => '{}' }));
    ctx.control.state = { mode: 'agent', mine: false };
    await call('send-chat', []);
    assert.deepEqual(ctx.control.changes, []);
    ctx.control.state = { mode: 'paused' };
    await call('send-chat', []);
    assert.deepEqual(ctx.control.changes, ['return']);
    fetch.mock.restore();
  });

  it('does not ask when the browser cannot be handed to the agent', async () => {
    ctx.config.values = { serverUrl: 'ws://s.test/ws', apiKey: 'k' };
    const fetch = mock.method(globalThis, 'fetch', async () => ({ status: 200, text: async () => '{}' }));
    ctx.control.change = async () => {
      throw new Error('Another operator has control');
    };
    assert.deepEqual(await call('send-chat', []), {
      error: 'Could not hand the browser to the agent: Another operator has control',
    });
    assert.equal(fetch.mock.callCount(), 0);
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
    ctx.channel = (name, ...args) => handlers.get(name)({}, ...args);
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
      // A queue that hands work the last task's result, as the old one did.
      queueRecording: (work) => work({ recording: false }),
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

  it('starts a recording afresh, never resuming whatever ran before', async () => {
    await ctx.channel('start-recording');
    assert.equal(ctx.resumed, undefined);
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
    ctx.workspace.snapshot = () => ({ draft: {} });
    ctx.electron.dialog.answers.saveDialog.push({ canceled: false, filePath: file });
    assert.deepEqual(await call({ type: 'support' }), { draft: {}, supportSaved: true });
    assert.deepEqual(JSON.parse(fs.readFileSync(file, 'utf8')), { ok: 1 });
    assert.equal(fs.statSync(file).mode & 0o777, 0o600);
  });

  it('says a cancelled diagnostics report was not saved', async () => {
    ctx.workspace.support = () => ({ ok: 1 });
    ctx.workspace.snapshot = () => ({ draft: {} });
    ctx.electron.dialog.answers.saveDialog.push({ canceled: true });
    assert.equal((await call({ type: 'support' })).supportSaved, false);
  });
});
