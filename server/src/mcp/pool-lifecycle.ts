/**
 * The pool's own tools: start and stop a browser, and show the pool.
 */
import { z } from 'zod';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { registry } from '../modules/browsers/registry.ts';
import { sendCommand } from '../modules/browsers/socket.ts';
import { control, projectId } from '../modules/control/service.ts';
import { ownsCloudBrowser, poolStats } from '../modules/browsers/pool.ts';
import {
  awaitBrowser,
  browserTag,
  hold,
  letGo,
  pendingBrowser,
  poolPinned,
  poolSticky,
  waitingMessage,
} from './pool-state.ts';
import { selfApi, type Self } from './self-api.ts';
import { attempt, fail, text } from './replies.ts';
import { MAX_NAME, NAVIGATE_TIMEOUT_MS, START_POLL_MS, START_WAIT_MS } from './constants.ts';
import { toolConfig } from './annotations.ts';

/** Whose pool the tools act on, and how they reach the public API. */
type Pool = {
  /** The caller's key. */
  apiKey: string;
  /** The caller's origin and credential. */
  self: Self;
};

/** Remote starts select only providers that allocate hosted browsers. */
const CLOUD_PROVIDERS = ['oya-cloud', 'oya-selfhosted', 'browserbase', 'steel', 'anchor', 'browseruse'] as const;

/** What start_browser takes. */
const START_SCHEMA = {
  persona: z
    .string()
    .optional()
    .describe("Persona id, 'auto' (least recently used under its concurrency cap) or 'default'"),
  provider: z.enum(CLOUD_PROVIDERS).optional(),
  name: z.string().max(MAX_NAME).optional(),
  url: z.string().optional().describe('Navigate here once the browser is ready'),
};

/** What start_browser is for. */
const START_DESCRIPTION =
  'Start a new browser on this key and make it the one every other tool drives. Defaults to Oya Cloud. It costs a session until stop_browser.';

/** Registers start_browser, stop_browser and pool_status. */
export function registerLifecycleTools(server: McpServer, pool: Pool) {
  const start = toolConfig('start_browser', START_DESCRIPTION, START_SCHEMA);
  server.registerTool('start_browser', start, (args) => startBrowserTool(pool, args));
  const stopDescription = 'Stop a browser and release its session. Defaults to the browser the tools are driving.';
  const stop = toolConfig('stop_browser', stopDescription, { browser_id: z.string().optional() });
  server.registerTool('stop_browser', stop, (args) => stopBrowserTool(pool, args));
  const status = toolConfig('pool_status', 'Show pool size and connected browsers.', {});
  server.registerTool('pool_status', status, async () => poolStatus(pool.apiKey));
}

/** Starts a browser through the API, waits for it to dial in, and holds it. */
async function startBrowserTool(pool: Pool, { persona, provider, name, url }) {
  const body = { profile: persona, provider: provider || 'oya-cloud', name };
  const outcome = await attempt(() => selfApi(pool.self, '/browsers/start', body));
  if ('error' in outcome) return fail(outcome.error.message);
  const started = outcome.value;
  return selectStarted(pool, started, url);
}

/** Reserve the new target immediately so a slow worker never redirects actions to the old one. */
async function selectStarted(pool: Pool, started, url) {
  awaitBrowser(pool.apiKey, started.id);
  if (!(await connects(started.id))) return booting(started);
  if (!ownsCloudBrowser(pool.apiKey, started.id)) return fail('Cloud browser unavailable.');
  if (poolSticky.get(pool.apiKey) !== started.id) return fail('A newer browser selection replaced this start.');
  hold(pool.apiKey, started.id);
  return startedAt(started.id, started, url);
}

/** A browser the start API answered with. */
interface Started {
  /** Its id. */
  id: string;
  /** Its name, when it was given one. */
  name?: string;
  /** Where it runs. */
  provider?: string;
}

/** Machine-readable identity lets subsequent presentation target the same browser. */
function startedResult(reply: ReturnType<typeof text>, started: Started) {
  return { ...reply, structuredContent: { browserId: started.id, name: started.name || '' } };
}

/** The id lets the host check this browser after it dials in. */
const booting = (started: Started) =>
  startedResult(
    text(`Started ${started.id} on ${started.provider}; it is still booting. Check pool_status, then retry.`),
    started,
  );

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
  const at = url ? `, at ${url}` : '';
  const ready = `${browserTag(id)} ready on ${started.provider} as persona ${started.persona}${at}.`;
  const note = 'Every tool now drives this browser. Call stop_browser when you are done.';
  return startedResult(text(`${ready} ${note}`), started);
}

/**
 * A booting or disconnected cloud worker can still be cancelled using its durable allocation.
 * A connected browser always needs proof from its current registration.
 */
async function stoppable(key: string, id: string) {
  if (registry.isConnected(id)) return ownsCloudBrowser(key, id);
  const row = await control().store.get('session', id);
  return (
    row?.project === projectId(key) &&
    Boolean(row.enrollmentHash || row.hostedProvider || row.cleanup?.kind === 'vendor')
  );
}

/** Stops the given browser, or the one the tools are driving. */
async function stopBrowserTool(pool: Pool, { browser_id }) {
  const id = browser_id || pendingBrowser(pool.apiKey) || poolPinned.get(pool.apiKey);
  if (!id) return fail('no browser to stop');
  if (!(await stoppable(pool.apiKey, id))) return fail('Cloud browser unavailable.');
  const outcome = await attempt(async () => (await selfApi(pool.self, '/browsers/stop', { ids: [id] })).results?.[0]);
  if ('error' in outcome) return fail(outcome.error.message);
  if (!outcome.value?.ok) return fail(outcome.value?.error || `could not stop ${id}`);
  letGo(pool.apiKey, id);
  return text(`Stopped ${id}.`);
}

/** The pool's size and browsers, the driven one starred. */
function poolStatus(apiKey) {
  const waiting = waitingMessage(apiKey);
  if (waiting) return text(waiting);
  const stats = poolStats(apiKey, true);
  const pinned = pendingBrowser(apiKey) || poolPinned.get(apiKey);
  const line = (b) => `  ${b.name} (${b.id})${b.id === pinned ? ' ★' : ''}, ${b.currentUrl || 'idle'}`;
  if (!stats.size) return text('Cloud browsers: 0. Use start_browser to start one.');
  return text(`Cloud browsers: ${stats.size}\n${stats.browsers.map(line).join('\n')}`);
}
