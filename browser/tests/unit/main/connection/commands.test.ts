/**
 * Unit tests for CommandRunner (src/main/connection/commands.ts) and the tab commands: results sent once, the
 * dialog race answered first and the late answer dropped, tab management,
 * recording and workflow playback.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { CommandRunner } from '../../../../src/main/connection/commands.ts';
import { resultSummary } from '../../../../src/main/connection/result-summary.ts';
import { TabManager } from '../../../../src/main/tabs/tabs.ts';
import { mainCtx } from '../../support/main-ctx.cjs';
import { FakeWebContents, flush } from '../../support/fakes.cjs';

describe('CommandRunner', () => {
  let ctx: any;
  beforeEach(() => {
    mock.timers.enable({ apis: ['setTimeout'] });
    ctx = mainCtx({ tabs: TabManager, commands: CommandRunner });
    ctx.actions = {
      runPageAction: async (id) => ctx.commands.sendResult(id, true, { url: 'u' }),
      waitForTabReady: async () => {},
    };
  });
  afterEach(() => mock.timers.reset());

  /** The cmd_result for command `id`. */
  const resultOf = (id) => ctx.socket.ofType('cmd_result').filter((m) => m.id === id);

  it('runs a page command only once the tab’s translation has been undone', async () => {
    ctx.tabs.createTab('https://a.test/');
    let restore!: () => void;
    const order: string[] = [];
    ctx.translation = {
      released: () => new Promise<void>((resolve) => (restore = () => (order.push('restored'), resolve()))),
    };
    ctx.actions.runPageAction = async (id) => (order.push('ran'), ctx.commands.sendResult(id, true, {}));
    const running = ctx.commands.handleCommand({ id: 't1', action: 'click' });
    await flush();
    assert.deepEqual(order, []);
    restore();
    await running;
    assert.deepEqual(order, ['restored', 'ran']);
  });

  it('says the browser is not ready outside browsing mode', async () => {
    ctx.shell.browsingMode = false;
    await ctx.commands.handleCommand({ id: 'c1', action: 'click' });
    assert.deepEqual(resultOf('c1'), [
      { type: 'cmd_result', id: 'c1', ok: false, data: null, error: 'Browser not ready' },
    ]);
  });

  it('needs an active tab for page actions', async () => {
    await ctx.commands.handleCommand({ id: 'c2', action: 'click' });
    assert.equal(resultOf('c2')[0].error, 'No active tab');
    ctx.tabs.createTab('https://a.test/');
    await ctx.commands.handleCommand({ id: 'c3', action: 'click' });
    assert.deepEqual(resultOf('c3')[0].data, { url: 'u' });
  });

  it('refuses an action that is not a string before any command map can read it as one', async () => {
    ctx.tabs.createTab('https://a.test/');
    let reached = false;
    ctx.actions.runPageAction = async () => (reached = true);
    for (const action of [['evaluate_raw'], ['open_tab'], { a: 1 }, 7]) {
      await ctx.commands.handleCommand({ id: 'n', action, params: { expression: '1' } });
    }
    assert.equal(reached, false);
    assert.equal(ctx.tabs.list.length, 1);
    assert.deepEqual(new Set(resultOf('n').map((m) => m.error)), new Set(['action must be a string']));
  });

  it('carries our tab_unprotected code to the server, and never an Electron or Node code', async () => {
    ctx.tabs.createTab('https://a.test/');
    const thrown = (code) => async () => {
      throw Object.assign(new Error('no'), { code });
    };
    ctx.actions.runPageAction = thrown('tab_unprotected');
    await ctx.commands.handleCommand({ id: 'u1', action: 'navigate' });
    ctx.actions.runPageAction = thrown('ERR_NAME_NOT_RESOLVED');
    await ctx.commands.handleCommand({ id: 'u2', action: 'navigate' });
    assert.equal(resultOf('u1')[0].code, 'tab_unprotected');
    assert.equal('code' in resultOf('u2')[0], false);
  });

  it('refuses open_tab on a tab that could not be protected, with its code', async () => {
    mock.method(console, 'error', () => {});
    mock.method(ctx.protection, 'resetTabCDP', () => {});
    ctx.protection.setupTabCDP = async () => false;
    await ctx.commands.handleCommand({ id: 'o1', action: 'open_tab', params: { url: 'https://b.test/' } });
    const [answer] = resultOf('o1');
    assert.deepEqual([answer.ok, answer.code], [false, 'tab_unprotected']);
    assert.match(answer.error, /could not be protected/);
  });

  it('turns a thrown action into an error result', async () => {
    ctx.tabs.createTab('https://a.test/');
    ctx.actions.runPageAction = async () => {
      throw new Error('boom');
    };
    await ctx.commands.handleCommand({ id: 'c4', action: 'click' });
    assert.equal(resultOf('c4')[0].error, 'boom');
  });

  it('open_tab opens only http(s) and about:blank, and makes no tab for anything else', async () => {
    for (const url of ['file:///etc/passwd', 'javascript:alert(1)', 'view-source:https://x.test']) {
      await ctx.commands.handleCommand({ id: 'f', action: 'open_tab', params: { url } });
    }
    assert.equal(ctx.tabs.list.length, 0);
    assert.deepEqual(
      new Set(resultOf('f').map((m) => m.error)),
      new Set(['Only http and https addresses, or about:blank, can be opened.']),
    );
  });

  it('open_tab with no address opens the start page, so a person never sees a bare about:blank', async () => {
    await ctx.commands.handleCommand({ id: 'h', action: 'open_tab' });
    assert.equal(ctx.tabs.list[0].home, true);
    assert.equal(resultOf('h')[0].ok, true);
  });

  it('answers a command from this machine to its caller, never to the server', async () => {
    const answer = await ctx.commands.runLocal('local-1', 'mcp-s1', 'open_tab', { url: 'https://a.test/' });
    assert.equal(answer.ok, true);
    assert.equal(answer.data.url, 'https://a.test/');
    assert.equal(resultOf('local-1').length, 0, 'nothing went to the server');
  });

  it('answers a local command even with no server connection', async () => {
    ctx.socket.open = false;
    const answer = await ctx.commands.runLocal('local-2', 'mcp-s1', 'list_tabs');
    assert.equal(answer.ok, true);
  });

  it('without server agents, an agent still never moves into the person’s background tabs', async () => {
    const mine = ctx.tabs.createTab('https://mine.test/', false);
    await ctx.commands.handleCommand({
      id: 'o',
      lane: 'mcp-a',
      action: 'open_tab',
      params: { url: 'https://a.test/' },
    });
    await ctx.commands.handleCommand({ id: 's', lane: 'mcp-a', action: 'switch_tab', params: { tab_id: mine } });
    assert.match(resultOf('s')[0].error, /belongs to the person/);
    await ctx.commands.handleCommand({ id: 'l', lane: 'mcp-a', action: 'list_tabs' });
    const yours = resultOf('l')[0]
      .data.tabs.filter((t) => t.yours)
      .map((t) => t.id);
    assert.equal(yours.includes(mine), false);
  });

  it('never closes a tab the person opened', async () => {
    const mine = ctx.tabs.createTab('https://mine.test/', true);
    await ctx.commands.handleCommand({ id: 'x', action: 'close_tab', params: { tab_id: mine } });
    await ctx.commands.handleCommand({ id: 'y', action: 'close_tab' });
    for (const id of ['x', 'y']) assert.match(resultOf(id)[0].error, /only tabs it opened itself/);
    assert.deepEqual(
      ctx.tabs.list.map((t) => t.id),
      [mine],
    );
  });

  it('never closes a tab another agent opened, only its own', async () => {
    await ctx.commands.handleCommand({
      id: 'a',
      lane: 'agent-a',
      action: 'open_tab',
      params: { url: 'https://a.test/' },
    });
    const theirs = resultOf('a')[0].data.tab_id;
    await ctx.commands.handleCommand({ id: 'b', lane: 'agent-b', action: 'close_tab', params: { tab_id: theirs } });
    assert.match(resultOf('b')[0].error, /only tabs it opened itself/);
    await ctx.commands.handleCommand({ id: 'c', lane: 'agent-a', action: 'close_tab', params: { tab_id: theirs } });
    assert.equal(resultOf('c')[0].ok, true);
    assert.equal(ctx.tabs.find(theirs), undefined);
  });

  it('opens, lists, switches and closes tabs', async () => {
    await ctx.commands.handleCommand({ id: 'o', action: 'open_tab', params: { url: 'https://a.test/' } });
    await ctx.commands.handleCommand({ id: 'o2', action: 'open_tab' });
    await ctx.commands.handleCommand({ id: 's', action: 'switch_tab', params: { tab_id: 1 } });
    await ctx.commands.handleCommand({ id: 'bad', action: 'switch_tab', params: { tab_id: 9 } });
    await ctx.commands.handleCommand({ id: 'l', action: 'list_tabs' });
    await ctx.commands.handleCommand({ id: 'x', action: 'close_tab' });
    assert.deepEqual(resultOf('o')[0].data, { tab_id: 1, url: 'https://a.test/' });
    assert.deepEqual(resultOf('o2')[0].data, { tab_id: 2, url: 'about:blank' });
    assert.equal(resultOf('bad')[0].error, 'Tab 9 not found');
    assert.deepEqual(
      resultOf('l')[0].data.tabs.map((t) => t.active),
      [true, false],
    );
    assert.deepEqual(
      ctx.tabs.list.map((t) => t.id),
      [2],
    );
  });

  it("keeps the agent on its own tab while the person uses another, and never takes the person's", async () => {
    ctx.control.state = { mode: 'agent', mine: false, interactive: false, connected: true, localClients: 0 };
    ctx.tabs.createTab('https://agent.test/');
    ctx.tabs.createTab('https://person.test/');
    ctx.tabs.activateTab(1);
    const driven: unknown[] = [];
    ctx.actions.runPageAction = async (id, _action, _params, view) => {
      driven.push(view);
      ctx.commands.sendResult(id, true, {});
    };
    await ctx.commands.handleCommand({ id: 'c1', action: 'click' });
    ctx.tabs.activateTab(2); // the person turns to their own tab
    await ctx.commands.handleCommand({ id: 'c2', action: 'click' });
    await ctx.commands.handleCommand({ id: 'sw', action: 'switch_tab', params: { tab_id: 2 } });
    await ctx.commands.handleCommand({ id: 'op', action: 'open_tab', params: { url: 'https://more.test/' } });
    await ctx.commands.handleCommand({ id: 'l', action: 'list_tabs' });
    const agentView = ctx.tabs.find(1).view;
    assert.deepEqual(driven, [agentView, agentView]);
    assert.equal(resultOf('sw')[0].error, 'The person is using that tab. Use another tab, or open_tab.');
    assert.equal(ctx.tabs.activeTabId, 2, 'the agent opening a tab never pulls the person off theirs');
    assert.equal(ctx.agentTab.id, 3, 'the tab the agent opened is its tab');
    assert.deepEqual(
      resultOf('l')[0]
        .data.tabs.filter((t) => t.active)
        .map((t) => t.id),
      [3],
    );
  });

  it("answers a held dialog at once and drops the interrupted command's late answer", async () => {
    const page = new FakeWebContents();
    ctx.dialogs.watch(page);
    let finish;
    ctx.tabs.createTab('https://a.test/');
    ctx.actions.runPageAction = (id) => new Promise((r) => (finish = () => r(ctx.commands.sendResult(id, true, {}))));
    const running = ctx.commands.handleCommand({ id: 'd', action: 'click' });
    await flush();
    page.nativeDialog('confirm', 'Delete?');
    await running;
    assert.match(resultOf('d')[0].error, /A JavaScript confirm dialog is open: "Delete\?"/);
    finish();
    await flush();
    assert.equal(resultOf('d').length, 1);
    await ctx.dialogs.answer(true);
  });

  it('attaches a dialog that fired during a command to its result', async () => {
    const page = new FakeWebContents();
    ctx.dialogs.watch(page);
    ctx.tabs.createTab('https://a.test/');
    ctx.actions.runPageAction = async (id) => {
      page.nativeDialog('alert', 'Saved');
      ctx.commands.sendResult(id, true, null);
    };
    await ctx.commands.handleCommand({ id: 'a', action: 'click' });
    assert.match(resultOf('a')[0].data.dialog, /Dialog \(alert\): "Saved"/);
  });

  it('sends nothing while the socket is down', () => {
    ctx.socket.open = false;
    ctx.commands.sendResult('z', true, {});
    assert.equal(ctx.socket.sent.length, 0);
  });

  it('runs recording commands through the recording queue', async () => {
    ctx.recorder = { queueRecording: (work) => work(), remote: async (mode) => ({ recording: mode === 'start' }) };
    await ctx.commands.handleCommand({ id: 'r', action: 'record', params: { mode: 'start' } });
    assert.deepEqual(resultOf('r')[0].data, { recording: true });
  });

  it('plays a workflow and reports its run', async () => {
    const workspace = {
      draft: {},
      run: { id: 'run', status: 'succeeded', assertions: 2 },
      persisted: 0,
      busy: () => false,
      persist() {
        this.persisted++;
      },
      async start(options) {
        this.options = options;
      },
    };
    ctx.workspace = workspace;
    ctx.recorder = {
      recording: false,
      adopt(steps) {
        this.steps = steps;
      },
    };
    await ctx.commands.handleCommand({
      id: 'w',
      action: 'workflow',
      params: { draft: { steps: [] }, variables: { a: 1 } },
    });
    assert.deepEqual(resultOf('w')[0].data, { id: 'run', status: 'succeeded', assertions: 2, error: undefined });
    assert.deepEqual(workspace.options, { vars: { a: 1 }, autoHeal: true });
    assert.equal(workspace.persisted, 2);
  });

  it('refuses a workflow while recording', async () => {
    ctx.workspace = { busy: () => false };
    ctx.recorder = { recording: true };
    await ctx.commands.handleCommand({ id: 'w2', action: 'workflow', params: {} });
    assert.match(resultOf('w2')[0].error, /Finish the active recording/);
  });
});

describe('resultSummary', () => {
  it('shows the shape of a result, not its payload', () => {
    const summary = resultSummary('abcdefghijkl', true, {
      screenshot: 'x'.repeat(2048),
      format: 'toon',
      page: 'm'.repeat(600),
      blocks: [1, 2, 3],
      elements: [1, 2],
      tabs: [1],
    });
    assert.deepEqual(summary, {
      id: 'abcdefgh',
      ok: true,
      screenshot: '2KB',
      format: 'toon',
      page: 'm'.repeat(500) + '...',
      blocks: '3 blocks',
      elements: '2 elements',
      tabs: '1 tabs',
    });
    assert.deepEqual(resultSummary('id', false, null, 'bad'), { id: 'id', ok: false, error: 'bad' });
  });
});
