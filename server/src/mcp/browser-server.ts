/**
 * The per-browser MCP server: the browser tools, all aimed at one browser.
 * Each browser gets its own endpoint: POST/GET/DELETE /mcp/:browserId
 */
import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { registerBrowserTools } from './browser-tools.ts';
import { registry } from '../modules/browsers/registry.ts';
import { Status } from '../platform/http-status.ts';
import { authorize } from './auth.ts';
import { answerFailure, serve } from './transport.ts';
import { MCP_VERSION } from './constants.ts';

/** The current-page resource's address. */
const CURRENT_PAGE = 'browser://current-page';

/** Create an MCP server for a browser session. */
function createMcpServer(browserId) {
  const browser = registry.get(browserId);
  const serverName = browser ? `Oya Browser, ${browser.name}` : `Oya Browser, ${browserId}`;
  const server = new McpServer({ name: serverName, version: MCP_VERSION });
  registerBrowserTools(server, { pick: () => browserId, oneBrowser: true });
  const description = 'Current URL and page title of the connected browser';
  server.resource('current-page', CURRENT_PAGE, { description }, async () => currentPage(browserId));
  return server;
}

/** Where the browser is, as the current-page resource reads. */
function currentPage(browserId) {
  const b = registry.get(browserId);
  const text = b ? `URL: ${b.currentUrl || '(unknown)'}\nBrowser: ${b.name}` : 'Browser not connected';
  return { contents: [{ uri: CURRENT_PAGE, text, mimeType: 'text/plain' }] };
}

/** Express handler for one browser's MCP endpoint. */
export async function handleMcpRequest(req, res) {
  const { browserId } = req.params;
  const caller = await authorize(req, res);
  if (!caller) return;
  if (!reachable(browserId, caller.key))
    return res.status(Status.NOT_FOUND).json({ error: `Browser ${browserId} not connected` });
  await serveBrowser(browserId, req, res);
}

/** Serves the request from a fresh server for the browser; a failure is logged and answered 500. */
async function serveBrowser(browserId, req, res) {
  try {
    await serve(createMcpServer(browserId), req, res);
  } catch (err) {
    console.error(`[mcp] Request error for browser ${browserId}:`, err.message);
    answerFailure(res, err);
  }
}

/**
 * Scope check, only the key that owns this browser can access its MCP.
 * 404 (not 403) so non-owners can't probe for browser existence.
 */
const reachable = (browserId, key) => registry.isConnected(browserId) && registry.belongsTo(browserId, key);
