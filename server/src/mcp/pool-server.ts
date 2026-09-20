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
import { browserTag, pickFor } from './pool-state.ts';
import { registerLifecycleTools } from './pool-lifecycle.ts';
import type { Self } from './self-api.ts';
import { JSON_INDENT, MCP_VERSION } from './constants.ts';

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
];

/** The pool-status resource's address. */
const POOL_STATUS = 'browser://pool-status';

/**
 * A pool MCP server for `apiKey`. `self` is the request's own origin and
 * Authorization header, which the lifecycle tools replay against the public API.
 */
function createPoolMcpServer(apiKey, self: Self = {}) {
  const server = new McpServer({ name: 'Oya Browser Pool', version: MCP_VERSION });
  const noBrowser = 'no browser is running. Call start_browser first.';
  registerBrowserTools(server, { pick: pickFor(apiKey), tag: browserTag, noBrowser, only: POOL_TOOLS });
  registerLifecycleTools(server, { apiKey, self });
  const description = 'Pool size and browser list';
  server.resource('pool-status', POOL_STATUS, { description }, async () => poolStatusResource(apiKey));
  return server;
}

/** The pool's stats as JSON. */
function poolStatusResource(apiKey) {
  const text = JSON.stringify(poolStats(apiKey), null, JSON_INDENT);
  return { contents: [{ uri: POOL_STATUS, text, mimeType: 'application/json' }] };
}

/** Express handler for the pool MCP endpoint. */
export async function handlePoolMcpRequest(req, res) {
  const header = req.headers.authorization;
  const caller = await authorize(req, res);
  if (caller) await servePool(caller.key, header, req, res);
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
