/**
 * Each key's cloud sandboxes, listed from the runtime. Sandbox lifetime is
 * independent of its WebSocket, so the inventory lives outside the command
 * registry: disconnected clients remain visible but never routable.
 */
import { settings as settingsFor } from './config.ts';
import { ownerTag, displayName } from './names.ts';
import { WORKERS } from './worker.ts';
import { sandboxEnv } from './tenancy.ts';
import { INVENTORY_MAX_KEYS, INVENTORY_TTL_MS, INVENTORY_WAIT_MS } from '../constants.ts';

/** Cached rows per owner tag, with when they were read and any refresh in flight. */
const inventory = new Map();

/**
 * How a sandbox without a live connection is listed. The placeholders are
 * filled per sandbox; the key order is the API's.
 */
const DISCONNECTED_ROW = {
  id: '',
  name: '',
  clientType: 'oya',
  provider: 'oya-cloud',
  persona: null,
  personaName: null,
  health: 'dead',
  currentUrl: '',
  connectedAt: null,
  lastSeen: null,
  commands: 0,
  errors: 0,
  pending: 0,
  lastCommandAt: null,
  lastError: '',
  streaming: false,
  sandboxState: '',
};

/** Drops a key's cached inventory, so the next listing reads it fresh. */
export function forgetInventory(owner) {
  inventory.delete(owner);
}

/**
 * This key's connected browsers plus its cloud sandboxes that are not connected, listed as dead.
 * The sandbox list is cached for 5s per key and waited on for at most 3s.
 */
export async function listSandboxBrowsers(apiKey, connected = []) {
  const settings = apiKey && settingsFor(sandboxEnv(apiKey));
  if (!settings) return connected;
  const owner = ownerTag(apiKey);
  const entry = entryFor(owner);
  if (Date.now() - entry.at > INVENTORY_TTL_MS && !entry.pending) entry.pending = refresh(entry, settings, owner);
  if (entry.pending) await waitAtMost(entry.pending, INVENTORY_WAIT_MS);
  return withConnected(entry.rows, connected);
}

/** The owner's cache entry, created (evicting the oldest past the cap) when missing. */
function entryFor(owner) {
  let entry = inventory.get(owner);
  if (entry) return entry;
  if (inventory.size >= INVENTORY_MAX_KEYS) inventory.delete(inventory.keys().next().value);
  entry = { rows: [], at: 0, pending: null };
  inventory.set(owner, entry);
  return entry;
}

/** Re-reads the owner's sandboxes; a failure keeps the old rows and is only logged. */
function refresh(entry, settings, owner) {
  return storeRows(entry, settings, owner)
    .catch((err) => {
      console.warn(`[sandbox] Inventory refresh failed: ${err.message}`);
    })
    .finally(() => {
      entry.at = Date.now();
      entry.pending = null;
    });
}

/** Reads the owner's sandboxes into the entry. */
async function storeRows(entry, settings, owner) {
  entry.rows = await loadRows(settings, owner);
}

/** The owner's live browser sandboxes on the key's runtime, as disconnected rows. */
async function loadRows(settings, owner) {
  const sandboxes = await WORKERS[settings.runtime].list(settings, owner);
  // Also verify locally: never trust an upstream filter for tenant isolation.
  return sandboxes.filter((sandbox) => isOwnedBrowser(sandbox, owner)).map(rowFor);
}

/** Whether the sandbox is a live browser sandbox of this owner, by its own labels. */
function isOwnedBrowser(sandbox, owner) {
  const labels = sandbox.labels || {};
  if (labels['oya-owner'] !== owner || labels['oya-browser'] !== 'true') return false;
  return !!labels['oya-browser-id'] && !['deleted', 'destroyed'].includes(sandbox.state);
}

/** A sandbox listed as a browser that is not connected. */
function rowFor(sandbox) {
  const labels = sandbox.labels || {};
  const id = labels['oya-browser-id'];
  const persona = labels['oya-persona'] || null;
  return { ...DISCONNECTED_ROW, id, name: displayName(labels['oya-name'], id), persona, ...stateOf(sandbox) };
}

/** What the row says about the sandbox's own state. */
function stateOf(sandbox) {
  return {
    lastError: `Browser disconnected · sandbox ${sandbox.state || 'available'}`,
    sandboxState: sandbox.state || 'unknown',
  };
}

/** Waits for the promise, but no longer than `ms`. */
async function waitAtMost(promise, ms) {
  let timer;
  await Promise.race([promise, new Promise((resolve) => (timer = setTimeout(resolve, ms)))]);
  clearTimeout(timer);
}

/** Sandbox rows overlaid with the connected browsers; a connected sandbox keeps its cloud provider. */
function withConnected(sandboxRows, connected) {
  const rows = new Map(sandboxRows.map((row) => [row.id, row]));
  for (const row of connected) rows.set(row.id, rows.has(row.id) ? { ...row, provider: 'oya-cloud' } : row);
  return [...rows.values()];
}
