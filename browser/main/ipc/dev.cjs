/** IPC: the dev panel's chat, page source and quick actions. */
const { canCallServer, postToBrowserApi } = require('../connection/server-api.cjs');
const { activePageSource } = require('../tabs/page-source.cjs');
const { renderPage, FORMATS } = require('../../scripts/page-render.cjs');
const { ERROR_PREVIEW_CHARS } = require('../connection/constants.cjs');

/** The server's JSON answer, or an error quoting what it sent instead. */
async function readChatAnswer(res) {
  const text = await res.text();
  try {
    return JSON.parse(text);
  } catch {
    return { error: `Server returned ${res.status}: ${text.slice(0, ERROR_PREVIEW_CHARS)}` };
  }
}

/** Posts to this browser's route on the server; answers its JSON, or `{ error }`. */
async function askServer(ctx, route, payload) {
  if (!canCallServer(ctx)) return { error: 'Not connected to server' };
  try {
    return await readChatAnswer(await postToBrowserApi(ctx, route, payload));
  } catch (err) {
    return { error: err.message };
  }
}

/**
 * Asking this browser's agent to do something is handing it the wheel. While a
 * person held control, or automation sat paused, every command the agent sent was
 * refused ("paused for human takeover") and the run spun until it gave up. So the
 * agent gets control for the run, and a person who held it gets it back after.
 * Answers whether to take it back.
 */
async function lendToAgent(ctx) {
  const state = ctx.control.snapshot();
  const mine = state.mode === 'human' && state.mine;
  if (mine || state.mode === 'paused') await ctx.control.change('return');
  return mine;
}

/** Sends the chat to the server's agent for this browser, with control lent to it; answers its reply or `{ error }`. */
async function sendChat(ctx, _e, messages) {
  if (!canCallServer(ctx)) return { error: 'Not connected to server' };
  const takeBack = await lendToAgent(ctx).catch((e) => e);
  if (takeBack instanceof Error) return { error: `Could not hand the browser to the agent: ${takeBack.message}` };
  try {
    return await askServer(ctx, 'chat', { messages });
  } finally {
    if (takeBack) await ctx.control.change('acquire').catch(() => {});
  }
}

/**
 * Saves the agent's latest run on this browser as a playbook named `name`. The
 * server keeps each browser's last run, so a body with only a name saves it.
 */
const saveChatPlaybook = (ctx, _e, name) => askServer(ctx, 'playbooks', { name });

/** A kept analysis written again in a known format (the Source pane's format switch); '' otherwise. */
function renderKeptPage(_ctx, _e, analysis, format) {
  if (!FORMATS.includes(format) || !Array.isArray(analysis?.blocks)) return '';
  return renderPage(analysis, format);
}

/** Channel → handler. */
const DEV_HANDLERS = {
  'send-chat': sendChat,
  'save-chat-playbook': saveChatPlaybook,
  'get-page-source': (ctx) => activePageSource(ctx),
  'render-page': renderKeptPage,
  'dev-action': (ctx, _e, action, params) => ctx.actions.runDevAction(action, params),
};

module.exports = { DEV_HANDLERS };
