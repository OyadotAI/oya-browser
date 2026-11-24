/**
 * The pool MCP server: the browser tools round-robined across every browser
 * sharing the caller's key, plus tools to start and stop browsers.
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

/** Every tool the pool endpoint serves: the per-browser subset plus its own three. */
export const POOL_TOOL_NAMES = [...POOL_TOOLS, 'start_browser', 'stop_browser', 'pool_status', 'show_browser'];

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

/** The pool's stats as JSON. */
function poolStatusResource(apiKey) {
  const text = JSON.stringify(poolStats(apiKey, true), null, JSON_INDENT);
  return { contents: [{ uri: POOL_STATUS, text, mimeType: 'application/json' }] };
}

/** Express handler for the pool MCP endpoint. */
export async function handlePoolMcpRequest(req, res) {
  const header = req.headers.authorization;
  if (!header && isDiscovery(req)) return servePool('', undefined, req, res);
  const caller = await authorize(req, res);
  if (caller) await servePool(caller.key, header, req, res);
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

/** Serves the request from a fresh pool server; a failure is logged and answered 500. */
async function servePool(apiKey, authorization, req, res) {
  try {
    const self = { origin: `http://127.0.0.1:${req.socket.localPort}`, authorization };
    await serve(createPoolMcpServer(apiKey, self), req, res);
  } catch (err) {
    console.error(`[mcp] Pool request error:`, err.message);
    answerFailure(res, err);
  }
}
