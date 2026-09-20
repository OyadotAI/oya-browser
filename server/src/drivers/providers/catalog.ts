/**
 * The hosted vendors this server knows, described by config rather than code,
 * plus the helpers that read their responses and API keys.
 */

/** Built-in vendor descriptions; OYA_BROWSER_PROVIDERS can override or add to them. */
const PRESETS = {
  anchor: {
    createUrl: 'https://api.anchorbrowser.io/v1/sessions',
    headers: (key) => ({ 'anchor-api-key': key, 'Content-Type': 'application/json' }),
    body: () => ({}),
    wsPath: ['data.cdp_url', 'data.websocket_url', 'cdp_url'],
    idPath: ['data.id', 'id'],
    deleteUrl: (id) => `https://api.anchorbrowser.io/v1/sessions/${id}`,
  },
  // https://docs.browserbase.com/reference/api/update-a-session
  browserbase: {
    createUrl: 'https://api.browserbase.com/v1/sessions',
    headers: (key) => ({ 'x-bb-api-key': key, 'Content-Type': 'application/json' }),
    body: (env) => (env.BROWSERBASE_PROJECT_ID ? { projectId: env.BROWSERBASE_PROJECT_ID } : {}),
    wsPath: ['connectUrl', 'data.connectUrl'],
    idPath: ['id', 'data.id'],
    deleteUrl: (id) => `https://api.browserbase.com/v1/sessions/${id}`,
    deleteMethod: 'POST',
    deleteBody: { status: 'REQUEST_RELEASE' },
  },
  // https://docs.browser-use.com/cloud/api-v2/browsers/create-browser-session
  browseruse: {
    createUrl: 'https://api.browser-use.com/api/v2/browsers',
    headers: (key) => ({ 'X-Browser-Use-API-Key': key, 'Content-Type': 'application/json' }),
    body: () => ({}),
    wsPath: ['cdpUrl', 'data.cdpUrl', 'cdp_url'],
    idPath: ['id', 'data.id'],
    deleteUrl: (id) => `https://api.browser-use.com/api/v2/browsers/${id}`,
    deleteMethod: 'PATCH',
    deleteBody: { action: 'stop' },
  },
  // https://docs.steel.dev/overview/authentication
  // https://github.com/steel-dev/steel-node/blob/main/src/resources/sessions/sessions.ts
  steel: {
    createUrl: 'https://api.steel.dev/v1/sessions',
    headers: (key) => ({ 'steel-api-key': key, 'Content-Type': 'application/json' }),
    body: () => ({}),
    wsPath: ['websocketUrl', 'data.websocketUrl', 'connectUrl'],
    idPath: ['id', 'data.id'],
    deleteUrl: (id) => `https://api.steel.dev/v1/sessions/${id}/release`,
    deleteMethod: 'POST',
    wsQueryKey: 'apiKey',
  },
};

/** Read a dotted path ("data.cdp_url") out of a vendor response. */
function dig(obj, path) {
  return path.split('.').reduce((o, k) => (o == null ? undefined : o[k]), obj);
}

/** The first path that holds a non-empty string; vendors move fields between API versions. */
export function firstPath(obj, paths) {
  for (const p of paths) {
    const v = dig(obj, p);
    if (typeof v === 'string' && v) return v;
  }
  return null;
}

/** Presets merged with any OYA_BROWSER_PROVIDERS override. */
export function catalog(env = process.env) {
  const overrides = readOverrides(env);
  const merged = {};
  for (const name of new Set([...Object.keys(PRESETS), ...Object.keys(overrides)])) {
    merged[name] = { ...(PRESETS[name] || {}), ...(overrides[name] || {}) };
  }
  return merged;
}

/** OYA_BROWSER_PROVIDERS as parsed JSON; empty (with an error logged) when it does not parse. */
function readOverrides(env) {
  if (!env.OYA_BROWSER_PROVIDERS) return {};
  try {
    return JSON.parse(env.OYA_BROWSER_PROVIDERS);
  } catch (e) {
    console.error('[providers] OYA_BROWSER_PROVIDERS is not valid JSON:', e.message);
    return {};
  }
}

/** The vendor's API key, from <NAME>_API_KEY. */
export const keyFor = (name, env) => env[`${name.toUpperCase()}_API_KEY`] || '';

/** The vendor's request headers for this key; config may give a function or a fixed object. */
export const headersFor = (cfg, key) => (typeof cfg.headers === 'function' ? cfg.headers(key) : cfg.headers);
