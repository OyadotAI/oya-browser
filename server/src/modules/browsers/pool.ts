/**
 * Browser pool, groups browsers by API key for round-robin dispatch
 * and cookie synchronization.
 */

import { registry } from './registry.ts';

/** Round-robin index per pool key */
const rrIndex = new Map();

/**
 * Get all browser IDs in a pool (same API key or fleet token).
 */
export function getPoolBrowsers(apiKey) {
  const ids = [];
  for (const [id, b] of registry.browsers) {
    if (b.apiKey === apiKey) ids.push(id);
  }
  return ids;
}

/**
 * Pick the next browser in the pool via round-robin.
 * Returns browserId or null if pool is empty.
 */
export function nextBrowser(apiKey) {
  const ids = getPoolBrowsers(apiKey);
  if (ids.length === 0) return null;

  const idx = (rrIndex.get(apiKey) || 0) % ids.length;
  rrIndex.set(apiKey, idx + 1);
  return ids[idx];
}

/**
 * Get pool stats.
 */
export function poolStats(apiKey) {
  const ids = getPoolBrowsers(apiKey);
  return {
    size: ids.length,
    browsers: ids.map((id) => {
      const b = registry.get(id);
      return { id, name: b?.name, currentUrl: b?.currentUrl };
    }),
  };
}
