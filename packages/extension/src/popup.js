/**
 * The popup page's glue: shows what popup-model decided and wires the
 * buttons. The rules themselves live in popup-model.js.
 */
import { CONSOLE_URL } from './constants.js';
import { Action, Messages } from './messages.js';
import { openPopup, sendLogins } from './popup-model.js';

/** The browser's extension namespace: `browser` on Firefox, `chrome` on Chrome. */
const api = globalThis.browser ?? globalThis.chrome;

/** The popup's elements, by id. */
const el = (id) => document.getElementById(id);

/** What the popup currently holds once ready: the key, the site and the profiles. */
let state = null;

/** The labels of the follow-up button, by action. */
const ACTION_LABELS = {
  [Action.SETTINGS]: 'Open settings',
  [Action.CONSOLE]: 'Open console',
  [Action.RETRY]: 'Try again',
  [Action.DONE]: 'Done',
};

/** What each follow-up button does. */
const ACTION_RUNS = {
  [Action.SETTINGS]: () => api.runtime.openOptionsPage(),
  [Action.CONSOLE]: () => api.tabs.create({ url: CONSOLE_URL }),
  [Action.RETRY]: () => start(),
  [Action.DONE]: () => window.close(),
};

/** Shows a message and its follow-up button, if it has one. */
function show(message) {
  el('status').textContent = message.text;
  const action = el('action');
  action.hidden = !Object.hasOwn(ACTION_LABELS, message.action);
  action.textContent = ACTION_LABELS[message.action] ?? '';
  action.dataset.action = message.action;
}

/** Fills the profile list, preselecting the last one used. */
function fillProfiles(profiles, selected) {
  const select = el('profile');
  select.replaceChildren(...profiles.map((p) => new Option(p.name || p.id, p.id, false, p.id === selected)));
}

/** Shows the site's address, the profile list and the send button for a ready popup. */
function showReady(ready) {
  fillProfiles(ready.profiles, ready.selected);
  el('profile-field').hidden = false;
  el('send').hidden = false;
  el('status').textContent = ready.hint?.text ?? '';
  el('action').hidden = true;
}

/** Shows what openPopup decided: a message, or the ready popup. */
function render(opened) {
  el('site').hidden = !opened.site;
  el('site').textContent = opened.site?.host ?? '';
  if (opened.ready) return showReady(opened);
  el('profile-field').hidden = true;
  el('send').hidden = true;
  show(opened.message);
}

/** Loads the popup: profiles first, with the list disabled until they arrive. */
async function start() {
  show(Messages.loading);
  el('profile').disabled = true;
  state = await openPopup(api, globalThis.fetch).catch(() => ({ message: Messages.offline }));
  el('profile').disabled = false;
  render(state);
}

/** The chosen profile, as `{ id, name }`. */
function chosenProfile() {
  return state.profiles.find((p) => p.id === el('profile').value) ?? state.profiles[0];
}

/** Sends the site's logins, keeping the button disabled while it runs, then returns focus to it. */
async function send() {
  const request = { key: state.key, site: state.site, profile: chosenProfile() };
  const pending = sendLogins(api, globalThis.fetch, request);
  el('send').disabled = true;
  show(Messages.sending);
  show(await pending);
  el('send').disabled = false;
  el('send').focus();
}

el('send').addEventListener('click', send);
el('action').addEventListener('click', (e) => ACTION_RUNS[e.currentTarget.dataset.action]?.());
el('open-settings').addEventListener('click', (e) => {
  e.preventDefault();
  api.runtime.openOptionsPage();
});
start();
