/**
 * Serving one MCP request over Streamable HTTP. Servers are built per request
 * (stateless), so nothing outlives the request.
 */
import { StreamableHTTPServerTransport } from '@modelcontextprotocol/sdk/server/streamableHttp.js';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { Status } from '../platform/http-status.ts';

/** Connects `server` to a stateless transport and lets it answer the request. */
export async function serve(server: McpServer, req, res) {
  const transport = new StreamableHTTPServerTransport({
    sessionIdGenerator: undefined, // stateless
  });
  await server.connect(transport);
  await transport.handleRequest(req, res, req.body);
}

/** A request that failed before answering gets a 500. */
export function answerFailure(res, err) {
  if (!res.headersSent) {
    res.status(Status.INTERNAL).json({ error: err.message });
  }
}
