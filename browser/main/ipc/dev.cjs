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

/** Sends the chat to the server's agent for this browser; answers its reply or `{ error }`. */
const sendChat = (ctx, _e, messages) => askServer(ctx, 'chat', { messages });

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
