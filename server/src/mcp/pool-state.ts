/**
 * Which browser a pool agent is driving, per key. Commands that start a new
 * page context advance the round-robin; the rest stay pinned to the last-used
 * browser so element IDs remain valid. A browser from start_browser is sticky.
 */
import { registry } from '../modules/browsers/registry.ts';
import { nextBrowser } from '../modules/browsers/pool.ts';

/** Pool pinned browser state: apiKey → browserId */
export const poolPinned = new Map();

/**
 * apiKey -> the browser start_browser made. An agent that started a browser
 * means to drive that one, so navigate stays on it instead of advancing the
 * round-robin into someone else's. Cleared by stop_browser or disconnect.
 */
export const poolSticky = new Map();

/** Deletes every entry of `map` whose value is `value`. */
function dropValue(map: Map<unknown, unknown>, value) {
  for (const [key, v] of map) if (v === value) map.delete(key);
}

/**
 * Clean up when browser disconnects. Servers are created per-request
 * (stateless), so there is nothing to destroy; only its pool pins are cleared.
 */
export function destroyMcpServer(browserId) {
  dropValue(poolPinned, browserId);
  dropValue(poolSticky, browserId);
}

/** Pick browser: sticky if set, else pinned if alive, else round-robin. */
export function pickFor(apiKey) {
  return (advance: boolean) => {
    const pinned = poolPinned.get(apiKey);
    if (pinned && registry.isConnected(pinned) && (!advance || poolSticky.get(apiKey) === pinned)) return pinned;
    const id = nextBrowser(apiKey);
    if (!id) return null;
    poolPinned.set(apiKey, id);
    return id;
  };
}

/** Makes `id` the browser every tool drives for this key until it is stopped. */
export function hold(apiKey, id) {
  poolPinned.set(apiKey, id);
  poolSticky.set(apiKey, id);
}

/** A stopped browser is no longer pinned or sticky for this key. */
export function letGo(apiKey, id) {
  if (poolPinned.get(apiKey) === id) poolPinned.delete(apiKey);
  if (poolSticky.get(apiKey) === id) poolSticky.delete(apiKey);
}

/** The prefix tool output uses to name a browser: `[name id]`, or `[id]` when it is gone. */
export function browserTag(id) {
  const b = registry.get(id);
  return b ? `[${b.name} ${id}]` : `[${id}]`;
}
