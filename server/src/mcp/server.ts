/**
 * The MCP servers' entry point: per-browser and pool endpoints over Streamable
 * HTTP, and the cleanup a disconnecting browser triggers. The work lives in
 * browser-server.ts, pool-server.ts and pool-state.ts.
 */

export { handleMcpRequest } from './browser-server.ts';
export { handlePoolMcpRequest } from './pool-server.ts';
export { destroyMcpServer } from './pool-state.ts';
