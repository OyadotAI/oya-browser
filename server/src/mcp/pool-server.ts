/**
 * The pool MCP server. An API key gets the browser tools round-robined across
 * every browser sharing it, plus tools to start and stop browsers. A chat app
 * signed in over OAuth (claude.ai, ChatGPT) gets the local MCP's model instead:
 * ten tools, one browser of its own that starts on first use, nothing to manage.
 * Endpoint: POST/GET/DELETE /mcp/pool
 */
import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { registerBrowserTools } from './browser-tools.ts';
import { poolStats } from '../modules/browsers/pool.ts';
import { authorize } from './auth.ts';
import { answerFailure, serve } from './transport.ts';
import { browserTag, pickFor, waitingMessage } from './pool-state.ts';
import { registerLifecycleTools } from './pool-lifecycle.ts';
import { registerBrowserCard } from './browser-card.ts';
import { acquire, type Seat } from './connector-seat.ts';
import type { Self } from './self-api.ts';
import { JSON_INDENT, MCP_VERSION, PUBLIC_METHODS } from './constants.ts';

/** Tabs stay per-browser: a pool agent opens pages with navigate instead. */
const POOL_TOOLS = [
  'analyze_page',
  'navigate',
  'click',
  'type',
  'screenshot',
  'press_key',
  'handle_dialog',
  'scroll',
  'wait',
  'click_coordinates',
  'mouse_move',
  'double_click',
  'keyboard_type',
  'drag',
  // Through the agent's own tool code (agent-tools.ts); each acts on the browser the last call used.
  'select_option',
  'run_script',
  'wait_for',
  'find',
  'hover',
  'go_back',
  'go_forward',
  'reload',
  'solve_captcha',
  'sign_in',
  'complete_mfa',
  'list_saved_logins',
  'list_playbooks',
  'run_playbook',
  'run_task',
];

/** Every tool the pool endpoint serves an API key: the per-browser subset plus its own three. */
export const POOL_TOOL_NAMES = [...POOL_TOOLS, 'start_browser', 'stop_browser', 'pool_status', 'show_browser'];

/**
 * A chat connector's tools, the local MCP's core: six that act (each one an "Always allow" in
 * the chat app, so as few as do the job) and four that only read. Back and forward ride on
 * navigate, switching on open_tab, and dialogs are answered for it.
 */
export const CONNECTOR_TOOL_NAMES = [
  'navigate',
  'click',
  'type',
  'press_key',
  'scroll',
  'open_tab',
  'analyze_page',
  'screenshot',
  'list_tabs',
  'wait_for',
];

/** What a chat connector's model is told, in the local MCP's words: how to start, read and act. */
const CONNECTOR_INSTRUCTIONS =
  'Oya Browser in the cloud: a real Chrome browser for this chat. Use it for any task that needs a web page (opening, reading, testing, filling in, signing in). open_tab with a URL to start; the person sees a live view of it. The browser starts itself on first use, which can take up to 2 minutes. analyze_page to read, then click/type by element id. Ids from the last analyze_page stay valid until you analyze again or the page navigates: fill a whole form from one analyze_page, and analyze again only after a click changes the page. navigate loads a URL in your tab, or takes "back" or "forward". open_tab also switches to a tab from list_tabs. Pop-up dialogs are dismissed for you and reported in the result.';

/** Tool calls cannot detect conversation boundaries, so the host presents the final view. */
const TURN_END =
  'After each browsing turn, call show_browser once with the ID of the browser used, after all page actions and screenshots. Skip this when the browser was stopped.';

/** Pool tool selection and its empty-state recovery. */
const POOL_OPTIONS = {
  only: POOL_TOOLS,
  screenshotCard: true,
};

/** The pool-status resource's address. */
const POOL_STATUS = 'browser://pool-status';

/**
 * A pool MCP server for `apiKey`. `self` is the request's own origin and
 * Authorization header, which the lifecycle tools replay against the public API.
 */
function createPoolMcpServer(apiKey, self: Self = {}) {
  const server = new McpServer({ name: 'Oya Browser Pool', version: MCP_VERSION }, { instructions: TURN_END });
  const noBrowser = waitingMessage(apiKey) || 'No cloud browsers are running. Use start_browser to start one.';
  registerBrowserTools(server, { ...POOL_OPTIONS, noBrowser, pick: pickFor(apiKey), tag: browserTag });
  registerLifecycleTools(server, { apiKey, self });
  registerBrowserCard(server, apiKey, self);
  const description = 'Pool size and browser list';
  server.resource('pool-status', POOL_STATUS, { description }, async () => poolStatusResource(apiKey));
  return server;
}

/**
 * A chat connector's server: its ten tools on its own browser, acquired on each call, and the
 * live card beside open_tab. A keyless listing gets this one too, so a chat app sees the tools
 * it will get. `seat` is empty for that listing: every call is refused before reaching it.
 */
function createConnectorServer(seat: Seat) {
  const server = new McpServer({ name: 'Oya Browser', version: MCP_VERSION }, { instructions: CONNECTOR_INSTRUCTIONS });
  const options = { only: CONNECTOR_TOOL_NAMES, screenshotCard: true, connector: true, tag: browserTag };
  registerBrowserTools(server, { ...options, pick: () => null, acquire: (start) => acquire(seat, start) });
  registerBrowserCard(server, seat.apiKey, seat.self, false);
  return server;
}

/** The pool's stats as JSON. */
function poolStatusResource(apiKey) {
  const text = JSON.stringify(poolStats(apiKey, true), null, JSON_INDENT);
  return { contents: [{ uri: POOL_STATUS, text, mimeType: 'application/json' }] };
}

/** Express handler for the pool MCP endpoint. */
export async function handlePoolMcpRequest(req, res) {
  const header = req.headers.authorization;
  if (!header && isDiscovery(req)) return servePool({ id: '', apiKey: '', self: {} }, req, res);
  const caller = await authorize(req, res);
  if (!caller) return;
  const self = { origin: `http://127.0.0.1:${req.socket.localPort}`, authorization: header };
  await servePool(caller.seat ? { id: caller.seat, apiKey: caller.key, self } : { apiKey: caller.key, self }, req, res);
}

/**
 * A single public JSON-RPC call (initialize, tools/list) that may be served
 * keyless. A key that is sent but wrong still gets 401, so a misconfigured
 * client is told; a batch always authenticates.
 */
function isDiscovery(req) {
  const body = req.body;
  return req.method === 'POST' && !!body && !Array.isArray(body) && PUBLIC_METHODS.has(body.method);
}

/** Who the pool serves: a connector's seat (with `id`), or an API key's pool (without). */
type Caller = Omit<Seat, 'id'> & Partial<Pick<Seat, 'id'>>;

/** The server for this caller: a connector's (or a keyless listing's), else the API key's pool. */
const serverFor = (caller: Caller) =>
  caller.id !== undefined ? createConnectorServer(caller as Seat) : createPoolMcpServer(caller.apiKey, caller.self);

/** Serves the request from a fresh server; a failure is logged and answered 500. */
async function servePool(caller: Caller, req, res) {
  try {
    await serve(serverFor(caller), req, res);
  } catch (err) {
    console.error(`[mcp] Pool request error:`, err.message);
    answerFailure(res, err);
  }
}
