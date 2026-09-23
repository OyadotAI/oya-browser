/**
 * Who may connect: a valid key, a browser id that is the caller's to use, and
 * a persona with a free slot. A refusal is a Rejection, which the connection
 * turns into a close code; anything else is a server-side failure.
 */
import { randomUUID } from 'crypto';
import { authenticateToken } from '../../auth/service.ts';
import { registry } from '../registry.ts';
import { destroyMcpServer } from '../../../mcp/server.ts';
import { container } from '../../../app/container.ts';
import { Status } from '../../../platform/http-status.ts';
import { failPending } from './commands.ts';
import { CloseCode, KEY_HINT_CHARS, MAX_CLOSE_REASON } from './constants.ts';

/** A refused connection: closed with `code`, counted as `outcome`, logged as `detail`. */
export class Rejection extends Error {
  /** WebSocket close code sent to the browser. */
  declare code: number;
  /** Metrics outcome label. */
  declare outcome: string;
  /** What the server logs about it. */
  declare detail: string;

  /** `reason` is the close reason sent to the browser. */
  constructor(code: number, reason: string, outcome: string, detail = reason) {
    super(reason);
    this.code = code;
    this.outcome = outcome;
    this.detail = detail;
  }
}

/** Who is connecting, once admitted. */
export interface Admitted {
  /** The key the browser registers under. */
  apiKey: string;
  /** Its browser id (minted when it has none). */
  browserId: string;
}

/** Checks the server is accepting and the key and browser id are the caller's; replaces a stale socket. */
export async function admit(msg): Promise<Admitted> {
  // Draining: finish what is in flight, accept nothing new, so an instance can restart without dropping sessions.
  if (registry.draining) throw new Rejection(CloseCode.DRAINING, 'Server draining', 'draining', 'server is draining');
  const apiKey = await keyFor(msg.api_key, msg.browser_id);
  const browserId = msg.browser_id || randomUUID();
  const existing = registry.get(browserId);
  if (existing && existing.apiKey !== apiKey) throw hijack(browserId);
  if (existing) replaceSocket(browserId, existing);
  return { apiKey, browserId };
}

/** Another tenant's browser id: refused, or any valid key could take over that browser. */
const hijack = (browserId: string) =>
  new Rejection(
    CloseCode.REJECTED,
    'browser_id registered to a different key',
    'id_conflict',
    `browser_id ${browserId} belongs to a different key`,
  );

/**
 * The key the browser may register under. An outage is retryable (it throws);
 * a bad or viewer credential is refused with the code that stops reconnects.
 */
async function keyFor(presented: string, browserId: string) {
  const principal = await principalOf(presented);
  if (mayRegister(principal, browserId)) return principal.key as string;
  const detail = `unknown API key ${hint(presented)}. It must be listed in API_KEYS or registered for an account.`;
  throw new Rejection(CloseCode.REJECTED, 'Invalid API key', 'invalid_key', detail);
}

/** Who the credential belongs to, or null if it is not valid; an auth outage throws. */
function principalOf(presented: string) {
  return authenticateToken(presented, { allowBrowser: true }).catch((e) => {
    if (e.status === Status.UNAVAILABLE) throw e;
    return null;
  });
}

/** Viewers never register; a managed browser's credential registers only its own session. */
const mayRegister = (principal, browserId: string) =>
  !!principal?.key &&
  principal.role !== 'viewer' &&
  !(principal.role === 'browser' && principal.sessionId !== browserId);

/** The end of a key, for the log. */
const hint = (key?: string) => (key ? `…${String(key).slice(-KEY_HINT_CHARS)}` : '(none sent)');

/** The same owner reconnected: the old socket's commands fail and it is closed. */
function replaceSocket(browserId: string, existing) {
  failPending(browserId, 'reconnected', 'Browser reconnected');
  try {
    existing.ws.close(CloseCode.REPLACED, 'Replaced by new connection');
  } catch {}
  registry.remove(browserId);
  destroyMcpServer(browserId);
}

/**
 * The persona the browser runs as, with its concurrency slot taken. Resolved
 * before registering, so a capped persona is refused before the browser appears.
 */
export function takePersona(apiKey: string, browserId: string, requested?: string, hostPlatform?: string) {
  try {
    const persona = resolvePersona(apiKey, browserId, requested, hostPlatform);
    return container.personas.acquire(persona, browserId);
  } catch (err) {
    const outcome = err.status === Status.TOO_MANY_REQUESTS ? 'persona_capped' : 'persona_unknown';
    throw new Rejection(CloseCode.CONTROL_REJECTED, err.message.slice(0, MAX_CLOSE_REASON), outcome, err.message);
  }
}

/**
 * The requested persona, or the key's default when this server does not hold it (lost
 * across a restart): refusing would lock the browser out, since it retries the same id forever.
 */
function resolvePersona(apiKey: string, browserId: string, requested?: string, hostPlatform?: string) {
  const first = { platform: hostPlatform };
  try {
    return container.personas.resolve(apiKey, requested, first);
  } catch (err) {
    if (err.status !== Status.NOT_FOUND) throw err;
    console.warn(`[ws] ${browserId}: unknown persona ${requested}, running as the default`);
    return container.personas.resolve(apiKey, 'default', first);
  }
}
