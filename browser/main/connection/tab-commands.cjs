/**
 * Server commands the main process answers itself rather than a page: tabs,
 * recording and workflow playback. Command → handler.
 */
const crypto = require('crypto');
const { normalizeDraft } = require('../../scripts/workflow.cjs');
const { sleep } = require('../input.cjs');
const { WORKFLOW_LIMIT_MS, WORKFLOW_POLL_MS } = require('./constants.cjs');

/** Loads the server's draft into the workspace as the one to play. */
function loadWorkflowDraft(ctx, params) {
  const workspace = ctx.workspace;
  workspace.persist();
  workspace.draft = normalizeDraft({ ...params.draft, id: crypto.randomUUID(), phase: 'paused' });
  workspace.history = [];
  workspace.future = [];
  workspace.persist();
  ctx.recorder.adopt(structuredClone(workspace.draft.steps), workspace.draft.secrets);
}

/** Waits for the run, and interrupts one that lost its connection or ran too long. */
async function waitForWorkflow(ctx) {
  const workspace = ctx.workspace;
  const started = Date.now();
  while (workspace.busy() && Date.now() - started < WORKFLOW_LIMIT_MS && ctx.socket.ready)
    await sleep(WORKFLOW_POLL_MS);
  if (!workspace.busy()) return;
  workspace.session?.dispose();
  const error = 'Remote validation disconnected or exceeded its time limit. Check the website before retrying.';
  workspace.receive({ type: 'finished', status: 'interrupted', error });
}

/** Only one recording or validation at a time. */
function assertCanPlay(ctx) {
  if (!ctx.workspace || ctx.workspace.busy() || ctx.recorder.recording) {
    throw new Error('Finish the active recording or validation before playing a workflow');
  }
}

/**
 * The server drives recording too, so a flow can be demonstrated from the
 * dashboard's live view. Both routes share one buffer: the panel here and the
 * dashboard show the same steps.
 */
async function playWorkflow(runner, id, params) {
  const { ctx } = runner;
  assertCanPlay(ctx);
  loadWorkflowDraft(ctx, params);
  await ctx.workspace.start({ vars: params.variables || {}, autoHeal: params.autoHeal !== false });
  await waitForWorkflow(ctx);
  const run = ctx.workspace.run;
  runner.sendResult(id, true, { id: run.id, status: run.status, assertions: run.assertions || 0, error: run.error });
}

/** Opens a tab and answers once it is ready. */
async function openTab(runner, id, params) {
  const { tabs, actions } = runner.ctx;
  const tabId = tabs.createTab(params?.url || 'about:blank', true);
  await actions.waitForTabReady(tabs.list.find((t) => t.id === tabId));
  runner.sendResult(id, true, { tab_id: tabId, url: params?.url || 'about:blank' });
}

/** Command → handler, for commands that do not need the active page. */
const TAB_COMMANDS = {
  workflow: playWorkflow,
  record: async (runner, id, params) => {
    const recorder = runner.ctx.recorder;
    const result = await recorder.queueRecording(() => recorder.remote(params?.mode));
    runner.sendResult(id, true, result);
  },
  list_tabs: (runner, id) => {
    const { list, activeTabId } = runner.ctx.tabs;
    const tabs = list.map((t) => ({ id: t.id, title: t.title, url: t.url, active: t.id === activeTabId }));
    runner.sendResult(id, true, { tabs });
  },
  read_console: (runner, id, params) => {
    const entries = runner.ctx.observer?.readConsole(params || {}) || [];
    runner.sendResult(id, true, { entries });
  },
  read_network: (runner, id, params) => {
    const requests = runner.ctx.observer?.readNetwork(params || {}) || [];
    runner.sendResult(id, true, { requests });
  },
  open_tab: openTab,
  switch_tab: (runner, id, params) => {
    if (!runner.ctx.tabs.find(params?.tab_id))
      return runner.sendResult(id, false, null, `Tab ${params?.tab_id} not found`);
    runner.ctx.tabs.activateTab(params.tab_id);
    runner.sendResult(id, true, { tab_id: params.tab_id });
  },
  close_tab: (runner, id, params) => {
    runner.ctx.tabs.closeTab(params?.tab_id || runner.ctx.tabs.activeTabId);
    runner.sendResult(id, true, { closed: true });
  },
};

module.exports = { TAB_COMMANDS };
