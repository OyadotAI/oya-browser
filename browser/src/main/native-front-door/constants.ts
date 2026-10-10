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
/** Reject conflicting aliases and wildcard legacy bindings instead of silently changing exposure. */
function configuredPort(env: NodeJS.ProcessEnv): string | undefined {
  const native = env.OYA_NATIVE_CDP_PORT,
    legacy = env.OYA_REMOTE_DEBUGGING_PORT === '0' ? undefined : env.OYA_REMOTE_DEBUGGING_PORT;
  if (native && legacy && Number(native) !== Number(legacy)) throw Error('Conflicting native CDP listener ports');
  if ((native || legacy) && env.OYA_REMOTE_DEBUGGING_HOST && env.OYA_REMOTE_DEBUGGING_HOST !== NATIVE_DOOR.host)
    throw Error('Native CDP listener requires 127.0.0.1; wildcard and remote hosts are unsupported');
  return native || legacy;
}
/** Environment is read once; the legacy listener alias never enables an upstream proxy. */
export function nativeDoorConfig(env: NodeJS.ProcessEnv): NativeConfig | null {
  const configured = configuredPort(env);
  if (!configured) return null;
  const port = Number(configured);
  if (!/^\d+$/.test(configured) || !Number.isInteger(port) || port > NATIVE_DOOR.maxPort)
    throw Error('Invalid native CDP port');
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
