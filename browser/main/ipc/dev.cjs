/** IPC: the dev panel's chat, page source and quick actions. */
const { canCallServer, postToBrowserApi, getFromApi, postToApi } = require('../connection/server-api.cjs');
const { activePageSource } = require('../tabs/page-source.cjs');
const { renderPage, FORMATS } = require('../../scripts/page-render.cjs');
const { ERROR_PREVIEW_CHARS } = require('../connection/constants.cjs');

/** What a chat the person stopped answers. */
const STOPPED = 'Stopped';
/** What a chat answers while the agent is already on another task (a routine, or another chat). */
const BUSY = 'The agent is busy with another task. Stop it first, or try again when it finishes.';

/** The server's JSON answer, or an error quoting what it sent instead. */
async function readChatAnswer(res) {
  const text = await res.text();
  try {
    return JSON.parse(text);
  } catch {
    return { error: `Server returned ${res.status}: ${text.slice(0, ERROR_PREVIEW_CHARS)}` };
  }
}

/** Posts to this browser's route on the server; answers its JSON, or `{ error }` (STOPPED when `signal` aborted it). */
async function askServer(ctx, route, payload, signal) {
  if (!canCallServer(ctx)) return { error: 'Not connected to server' };
  try {
    return await readChatAnswer(await postToBrowserApi(ctx, route, payload, undefined, signal));
  } catch (err) {
    return { error: signal?.aborted ? STOPPED : err.message };
  }
}

/**
 * Runs `work(signal)` as the one chat in flight, with a signal Stop can abort.
 * Hanging up is the stop: the server sees the connection close and ends the run
 * at its next step. A second chat (or a routine) while one runs is refused, as
 * two runs would fight over the same page.
 */
async function asOnlyChat(ctx, work) {
  if (ctx.chatAbort) return { error: BUSY };
  const abort = (ctx.chatAbort = new AbortController());
  try {
    return await work(abort.signal);
  } finally {
    if (ctx.chatAbort === abort) ctx.chatAbort = null;
  }
}

/** Stops the chat in flight, if any; answers whether there was one. */
function stopChat(ctx) {
  ctx.chatAbort?.abort();
  return !!ctx.chatAbort;
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

/** Sends the chat (and any attached files, as `data`) to the server's agent for this browser, with control lent to it; answers its reply or `{ error }`. */
function sendChat(ctx, _e, messages, data) {
  if (!canCallServer(ctx)) return Promise.resolve({ error: 'Not connected to server' });
  const payload = data ? { messages, data } : { messages };
  return asOnlyChat(ctx, (signal) => withAgentControl(ctx, () => askServer(ctx, 'chat', payload, signal)));
}

/** Runs `work` with control lent to the agent, handing it back to the person after. */
async function withAgentControl(ctx, work) {
  const takeBack = await lendToAgent(ctx).catch((e) => e);
  if (takeBack instanceof Error) return { error: `Could not hand the browser to the agent: ${takeBack.message}` };
  try {
    return await work();
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

/**
 * What Ask offers against a server too old to send its catalog: the providers
 * it has always taken, each on its default model (any other is typed by hand).
 */
const LEGACY_CATALOG = [
  ['anthropic', 'Claude', 'sk-ant-...', 'https://console.anthropic.com/settings/keys', 'claude-opus-5'],
  ['openai', 'OpenAI', 'sk-...', 'https://platform.openai.com/api-keys', 'gpt-4o-mini'],
  ['gemini', 'Gemini', 'AIza...', 'https://aistudio.google.com/apikey', 'gemini-3.8-flash'],
].map(([id, label, hint, keysUrl, model]) => ({
  id,
  label,
  hint,
  keysUrl,
  model,
  models: [{ id: model, label: model }],
}));

/** The server's provider catalog, or the legacy one from a server that has none. */
const catalogOf = (config) => (config.llm_catalog?.length ? config.llm_catalog : LEGACY_CATALOG);

/** The provider a key runs on: the one it saved, else the catalog entry whose endpoint it is using. */
function currentProvider(config) {
  if (config.llm_provider) return config.llm_provider;
  return catalogOf(config).find((p) => p.base && p.base === config.effective?.baseUrl)?.id || '';
}

/** What the model card needs from the server's config. */
function statusOf(config) {
  const { hasLlmKey, model } = config.effective || {};
  return { hasLlmKey: !!hasLlmKey, provider: currentProvider(config), model, catalog: catalogOf(config) };
}

/** Whether this browser has a project, and the project's model as the server has it; unsure counts as having one. */
async function modelStatus(ctx) {
  const signedIn = !!ctx.config.values.apiKey;
  if (!canCallServer(ctx)) return { signedIn, hasLlmKey: true };
  const config = await getFromApi(ctx, 'config').catch(() => null);
  return config ? { signedIn, ...statusOf(config) } : { signedIn, hasLlmKey: true };
}

/**
 * The POST /config body for a choice made in Ask, or `{ error }`. The model is
 * always sent (never nulled behind the person's back), a key only when one was
 * typed, and the endpoint is reset only when the provider changes, so saving
 * here never undoes a model or gateway chosen in the console.
 */
function modelUpdate(config, { provider, model, key } = {}) {
  const secret = typeof key === 'string' ? key.trim() : '';
  if (!catalogOf(config).some((p) => p.id === provider)) return { error: 'Pick a provider.' };
  const changed = provider !== currentProvider(config);
  if (!secret && (changed || !config.effective?.hasLlmKey)) return { error: 'Paste your API key for this provider.' };
  const body = { llm_provider: provider, chat_model: (typeof model === 'string' && model.trim()) || null };
  if (secret) body.openai_api_key = secret;
  if (changed) body.openai_base_url = null;
  return body;
}

/** Saves the provider, model and (when given) key chosen in Ask to the project, checked against what the server has now. */
async function saveModelKey(ctx, _e, choice) {
  if (!canCallServer(ctx)) return { error: 'Not connected to server' };
  const config = await getFromApi(ctx, 'config').catch(() => null);
  if (!config) return { error: 'Could not read your project settings. Try again.' };
  const body = modelUpdate(config, choice);
  return body.error ? body : saveConfig(ctx, body);
}

/** Posts settings, as `{ ok }` or the server's reason. */
const saveConfig = (ctx, body) =>
  postToApi(ctx, 'config', body).then(
    () => ({ ok: true }),
    (e) => ({ error: e.message }),
  );

/** Channel → handler. */
const DEV_HANDLERS = {
  'model-status': modelStatus,
  'save-model-key': saveModelKey,
  'send-chat': sendChat,
  'stop-chat': stopChat,
  'save-chat-playbook': saveChatPlaybook,
  'get-page-source': (ctx) => activePageSource(ctx),
  'render-page': renderKeptPage,
  'dev-action': (ctx, _e, action, params) => ctx.actions.runDevAction(action, params),
};

module.exports = { DEV_HANDLERS, STOPPED, sendChat };
