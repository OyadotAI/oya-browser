/** The local MCP port's fixed values: where it listens, its limits, and the protocol it speaks. */

/** Only this machine can reach the port. */
export const MCP_HOST = '127.0.0.1';

/** The port agents are configured with when OYA_MCP_PORT does not move it. */
const DEFAULT_MCP_PORT = 47_321;

/** The port agents are configured with; OYA_MCP_PORT moves it when something else holds it. */
export const MCP_PORT = Number(process.env.OYA_MCP_PORT) || DEFAULT_MCP_PORT;

/** The path the MCP endpoint answers on. */
export const MCP_PATH = '/mcp';

/** The MCP protocol revision this server speaks (streamable HTTP, JSON responses). */
export const MCP_PROTOCOL_VERSION = '2025-06-18';

/** The largest request body read; an agent's tool call is a few hundred bytes. */
export const MCP_MAX_BODY_BYTES = 1_048_576;

/** How long one tool call may run before the agent is told it timed out. */
export const MCP_CALL_TIMEOUT_MS = 120_000;

/** How many agent sessions are remembered; the oldest is forgotten past this. */
export const MCP_MAX_SESSIONS = 64;

/** Random bytes in the bearer token agents send. */
export const MCP_TOKEN_BYTES = 32;

/** Random bytes in a session id. */
export const MCP_SESSION_BYTES = 16;

/** Where the token is kept, sealed with the OS keychain, in the app's data folder. */
export const MCP_TOKEN_FILE = 'local-mcp-token.enc';

/** The name each agent's config lists Oya under. */
export const MCP_SERVER_NAME = 'oya-browser';
/** Agent setup commands must release the first-run UI if a CLI stalls. */
export const AGENT_CLI_TIMEOUT_MS = 30_000;
/** Native app launch and terminal readiness may not hold onboarding indefinitely. */
export const AGENT_OPEN_TIMEOUT_MS = 10_000;
/** Only a short local marker is polled while the terminal starts. */
export const AGENT_OPEN_POLL_MS = 100;
/** Temporary terminal handoffs are private and executable only by this OS user. */
export const AGENT_LAUNCH_MODE = 0o700;

/** HTTP statuses the port answers with. */
export const Status = {
  OK: 200,
  ACCEPTED: 202,
  BAD_REQUEST: 400,
  UNAUTHORIZED: 401,
  FORBIDDEN: 403,
  NOT_FOUND: 404,
  METHOD_NOT_ALLOWED: 405,
  PAYLOAD_TOO_LARGE: 413,
} as const;

/** JSON-RPC error codes. */
export const RpcError = {
  PARSE: -32_700,
  INVALID_REQUEST: -32_600,
  METHOD_NOT_FOUND: -32_601,
  INVALID_PARAMS: -32_602,
};

/** The keys press_key may press: navigation only, never modifiers or F-keys (the hosted tools allow the same). */
export const SAFE_KEYS = [
  'Enter',
  'Escape',
  'Tab',
  'ArrowDown',
  'ArrowUp',
  'ArrowLeft',
  'ArrowRight',
  'Backspace',
  'Delete',
  'Space',
  'Home',
  'End',
  'PageUp',
  'PageDown',
];
