/** Bounded resources and explicit protocol errors for the native CDP front door. */
export const NATIVE_DOOR = {
  /** Local-only listener; never wildcard-bound. */
  host: '127.0.0.1',
  /** Highest TCP port. */
  maxPort: 65535,
  /** Prevent unbounded queued commands from a stalled caller. */
  maxQueued: 32,
  /** Bound per-socket attachment and event subscription state. */
  maxSessions: 128,
  /** WebSocket internal failure; no silent partial auto-attachment after a resource failure. */
  failedSocket: 1011,
  /** Maximum request size; screenshots are responses, not requests. */
  maxPayload: 1_048_576,
  /** Minimum bearer token length for a manually configured endpoint. */
  tokenLength: 32,
  /** Generic JSON-RPC operation error. */
  error: -32000,
  /** JSON-RPC missing method error. */
  unsupported: -32601,
  /** JSON-RPC malformed command error. */
  invalid: -32600,
  /** HTTP outcomes. */
  ok: 200,
  /** Authentication and Origin/Host refusal. */
  forbidden: 403,
  /** No such native endpoint. */
  missing: 404,
  /** HTTP method is not read-only discovery. */
  method: 405,
} as const;
/** Environment is read once at the native composition root, not by page commands. */
export function nativeDoorConfig(env: NodeJS.ProcessEnv): NativeConfig | null {
  if (!env.OYA_NATIVE_CDP_PORT) return null;
  const port = Number(env.OYA_NATIVE_CDP_PORT);
  if (!Number.isInteger(port) || port < 0 || port > NATIVE_DOOR.maxPort) throw Error('Invalid OYA_NATIVE_CDP_PORT');
  const token = env.OYA_NATIVE_CDP_TOKEN || '';
  if (token.length < NATIVE_DOOR.tokenLength) throw Error('OYA_NATIVE_CDP_TOKEN must contain at least 32 characters');
  return { port, token };
}

/** Explicit credentials for one loopback listener. */
interface NativeConfig {
  /** Requested TCP port. */
  port: number;
  /** Local bearer credential. */
  token: string;
}
