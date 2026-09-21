/**
 * The pool's own tools: start and stop a browser, and show the pool.
 */
import { z } from 'zod';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { registry } from '../modules/browsers/registry.ts';
import { sendCommand } from '../modules/browsers/socket.ts';
import { poolStats } from '../modules/browsers/pool.ts';
import { browserTag, hold, letGo, poolPinned } from './pool-state.ts';
import { selfApi, type Self } from './self-api.ts';
import { attempt, fail, text } from './replies.ts';
import { MAX_NAME, NAVIGATE_TIMEOUT_MS, START_POLL_MS, START_WAIT_MS } from './constants.ts';

/** Whose pool the tools act on, and how they reach the public API. */
type Pool = {
  /** The caller's key. */
  apiKey: string;
  /** The caller's origin and credential. */
  self: Self;
};

/** What start_browser takes. */
const START_SCHEMA = {
  persona: z
    .string()
    .optional()
    .describe("Persona id, 'auto' (least recently used under its concurrency cap) or 'default'"),
  provider: z.enum(['oya-cloud', 'oya-selfhosted', 'browserbase', 'steel', 'anchor', 'browseruse']).optional(),
  name: z.string().max(MAX_NAME).optional(),
  url: z.string().optional().describe('Navigate here once the browser is ready'),
};

/** What start_browser is for. */
const START_DESCRIPTION =
  "Start a new browser on this key and make it the one every other tool drives. Uses the key's configured provider unless one is given. It costs a session until stop_browser.";

/** Registers start_browser, stop_browser and pool_status. */
export function registerLifecycleTools(server: McpServer, pool: Pool) {
  server.tool('start_browser', START_DESCRIPTION, START_SCHEMA, (args) => startBrowserTool(pool, args));
  const stopDescription = 'Stop a browser and release its session. Defaults to the browser the tools are driving.';
  const stopSchema = { browser_id: z.string().optional() };
  server.tool('stop_browser', stopDescription, stopSchema, (args) => stopBrowserTool(pool, args));
  server.tool('pool_status', 'Show pool size and connected browsers.', {}, async () => poolStatus(pool.apiKey));
}

/** Starts a browser through the API, waits for it to dial in, and holds it. */
async function startBrowserTool(pool: Pool, { persona, provider, name, url }) {
  const outcome = await attempt(() => selfApi(pool.self, '/browsers/start', { profile: persona, provider, name }));
  if ('error' in outcome) return fail(outcome.error.message);
  const started = outcome.value;
  if (!(await connects(started.id)))
    return text(`Started ${started.id} on ${started.provider}; it is still booting. Check pool_status, then retry.`);
  hold(pool.apiKey, started.id);
  return startedAt(started.id, started, url);
}

/** Cloud browsers dial in after they boot; the tools need it connected. */
async function connects(id) {
  const deadline = Date.now() + START_WAIT_MS;
  while (!registry.isConnected(id) && Date.now() < deadline) await new Promise((r) => setTimeout(r, START_POLL_MS));
  return registry.isConnected(id);
}

/** Navigates to `url` when one was given, then says the browser is ready. */
async function startedAt(id, started, url) {
  if (url) {
    const nav = await sendCommand(id, 'navigate', { url }, NAVIGATE_TIMEOUT_MS);
    if (!nav.ok) return fail(`${browserTag(id)} started, but navigating failed: ${nav.error}`);
  }
  return text(
    `${browserTag(id)} ready on ${started.provider} as persona ${started.persona}${url ? `, at ${url}` : ''}. ` +
      'Every tool now drives this browser. Call stop_browser when you are done.',
  );
}

/** Stops the given browser, or the one the tools are driving. */
async function stopBrowserTool(pool: Pool, { browser_id }) {
  const id = browser_id || poolPinned.get(pool.apiKey);
  if (!id) return fail('no browser to stop');
  const outcome = await attempt(async () => (await selfApi(pool.self, '/browsers/stop', { ids: [id] })).results?.[0]);
  if ('error' in outcome) return fail(outcome.error.message);
  const result = outcome.value;
  if (result && result.ok === false) return fail(result.error || `could not stop ${id}`);
  letGo(pool.apiKey, id);
  return text(`Stopped ${id}.`);
}

/** The pool's size and browsers, the driven one starred. */
function poolStatus(apiKey) {
  const stats = poolStats(apiKey);
  const pinned = poolPinned.get(apiKey);
  const line = (b) => `  ${b.name} (${b.id})${b.id === pinned ? ' ★' : ''}, ${b.currentUrl || 'idle'}`;
  return text(`Pool: ${stats.size} browsers\n${stats.browsers.map(line).join('\n')}`);
}
