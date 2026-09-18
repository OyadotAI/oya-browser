/**
 * Unit tests for CommandRunner and the tab commands: results sent once, the
 * dialog race answered first and the late answer dropped, tab management,
 * recording and workflow playback.
 */
const { describe, it, beforeEach, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { CommandRunner } = require('../../../../main/connection/commands.cjs');
const { resultSummary } = require('../../../../main/connection/result-summary.cjs');
const { TabManager } = require('../../../../main/tabs/tabs.cjs');
const { attachDialogWatcher, answerDialog } = require('../../../../main/dialogs.cjs');
const { mainCtx } = require('../../support/main-ctx.cjs');
const { FakeDebugger, flush } = require('../../support/fakes.cjs');

describe('CommandRunner', () => {
  let ctx;
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

  it('turns a thrown action into an error result', async () => {
    ctx.tabs.createTab('https://a.test/');
    ctx.actions.runPageAction = async () => {
      throw new Error('boom');
    };
    await ctx.commands.handleCommand({ id: 'c4', action: 'click' });
    assert.equal(resultOf('c4')[0].error, 'boom');
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

  it("answers a held dialog at once and drops the interrupted command's late answer", async () => {
    const dbg = new FakeDebugger();
    attachDialogWatcher(dbg);
    let finish;
    ctx.tabs.createTab('https://a.test/');
    ctx.actions.runPageAction = (id) => new Promise((r) => (finish = () => r(ctx.commands.sendResult(id, true, {}))));
    const running = ctx.commands.handleCommand({ id: 'd', action: 'click' });
    await flush();
    dbg.event('Page.javascriptDialogOpening', { type: 'confirm', message: 'Delete?' });
    await running;
    assert.match(resultOf('d')[0].error, /A JavaScript confirm dialog is open: "Delete\?"/);
    finish();
    await flush();
    assert.equal(resultOf('d').length, 1);
    await answerDialog(true);
  });

  it('attaches a dialog that fired during a command to its result', async () => {
    const dbg = new FakeDebugger();
    attachDialogWatcher(dbg);
    ctx.tabs.createTab('https://a.test/');
    ctx.actions.runPageAction = async (id) => {
      dbg.event('Page.javascriptDialogOpening', { type: 'alert', message: 'Saved' });
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
      markdown: 'm'.repeat(600),
      elements: [1, 2],
      tabs: [1],
    });
    assert.deepEqual(summary, {
      id: 'abcdefgh',
      ok: true,
      screenshot: '2KB',
      markdown: 'm'.repeat(500) + '...',
      elements: '2 elements',
      tabs: '1 tabs',
    });
    assert.deepEqual(resultSummary('id', false, null, 'bad'), { id: 'id', ok: false, error: 'bad' });
  });
});
