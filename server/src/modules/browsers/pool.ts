/**
 * Browser pool, groups browsers by API key for round-robin dispatch
 * and cookie synchronization.
 */

import { registry } from './registry.ts';

/** Cloud MCP targets must prove worker enrollment or originate in a hosted provider allocation. */
export const ownsCloudBrowser = (key: string, id: string) =>
  registry.belongsTo(id, key) && registry.get(id).cloudVerified;

/** Round-robin index per pool key */
const rrIndex = new Map();

/**
 * Get all browser IDs in a pool (same API key or fleet token).
 */
export function getPoolBrowsers(apiKey, cloudOnly = false) {
  const ids = [];
  for (const [id, b] of registry.browsers) {
    if (b.apiKey === apiKey && (!cloudOnly || b.cloudVerified)) ids.push(id);
  }
  return ids;
}

/**
 * Pick the next browser in the pool via round-robin.
 * Returns browserId or null if pool is empty.
 */
export function nextBrowser(apiKey, cloudOnly = false) {
  const ids = getPoolBrowsers(apiKey, cloudOnly);
  if (ids.length === 0) return null;

  const idx = (rrIndex.get(apiKey) || 0) % ids.length;
  rrIndex.set(apiKey, idx + 1);
  return ids[idx];
}

/**
 * Get pool stats.
 */
export function poolStats(apiKey, cloudOnly = false) {
  const ids = getPoolBrowsers(apiKey, cloudOnly);
  return {
    size: ids.length,
    browsers: ids.map((id) => {
      const b = registry.get(id);
      return { id, name: b?.name, currentUrl: b?.currentUrl };
    }),
  };
}
