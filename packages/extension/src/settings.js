/**
 * The settings page's glue: shows the saved key, checks a new one against
 * Oya, and keeps it only when Oya accepts it.
 */
import { StorageKey } from './constants.js';
import { keyRefusedMessage, keySavedMessage } from './messages.js';
import { saveKey } from './popup-model.js';

/** The browser's extension namespace: `browser` on Firefox, `chrome` on Chrome. */
const api = globalThis.browser ?? globalThis.chrome;

/** The page's elements, by id. */
const el = (id) => document.getElementById(id);

/** Checks and saves the key typed in, then says what happened. */
async function save(event) {
  event.preventDefault();
  const key = el('key').value.trim();
  if (!key) return;
  el('save').disabled = true;
  el('status').textContent = await saveKey(api, globalThis.fetch, key).then(keySavedMessage, keyRefusedMessage);
  el('save').disabled = false;
}

/** Shows or hides the key as typed. */
function toggleShow() {
  const shown = el('key').type === 'text';
  el('key').type = shown ? 'password' : 'text';
  el('show').textContent = shown ? 'Show' : 'Hide';
  el('show').setAttribute('aria-pressed', String(!shown));
}

/** Puts the saved key, if any, back in the field. */
async function load() {
  const stored = await api.storage.local.get(StorageKey.API_KEY);
  el('key').value = stored[StorageKey.API_KEY] ?? '';
}

el('key-form').addEventListener('submit', save);
el('show').addEventListener('click', toggleShow);
load();
