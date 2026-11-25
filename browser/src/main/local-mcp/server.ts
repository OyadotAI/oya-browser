/**
 * Oya's MCP port on this computer: Claude Code, Codex and other agents connect
 * to http://127.0.0.1:<port>/mcp with a bearer token and drive the browser with
 * the agent loop's own commands. Each session is its own agent (lane), so it
 * works in its own tab; Human Mode pauses it as it pauses every agent.
 *
 * Only this machine can reach it, every request needs the token, and a request
 * from a web page (it carries an Origin) or for another host name is refused, so
 * a site cannot drive the browser through it.
 */
import http, { type IncomingMessage, type ServerResponse } from 'node:http';
import { randomBytes, randomUUID, timingSafeEqual } from 'node:crypto';
import type { AppServices } from '../app/services.ts';
import { answer } from './protocol.ts';
import { inLane } from '../control/agent-tab.ts';
import type { LocalResult } from '../connection/commands.ts';
import { ensureToken } from './token.ts';
import * as C from './constants.ts';
import { releaseViewport, releaseAllViewports } from '../actions/viewport.ts';

/** What the port uses: the command runner, control, agent tabs, and Electron for the token's keychain. */
type Deps = Pick<AppServices, 'commands' | 'control' | 'electron' | 'agentTab'>;

/** A finished HTTP answer. */
interface Reply {
  /** Its status. */
  status: number;
  /** Its JSON body, if any. */
  body?: object;
  /** Extra headers. */
  headers?: Record<string, string>;
}

/** The MCP port. */
export class LocalMcp {
  /** The services it drives. */
  private readonly deps: Deps;
  /** The listening server, once started. */
  private server: http.Server | null = null;
  /** The bearer token agents send. */
  private secret = '';
  /** Whether this process holds the port; a port another program holds must never be handed the token. */
  private bound = false;

  /** `deps` is the main-process services. */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** The address agents are configured with. */
  get url(): string {
    return `http://${C.MCP_HOST}:${C.MCP_PORT}${C.MCP_PATH}`;
  }

  /** The token agents are configured with; empty unless this process is the one listening. */
  get token(): string {
    return this.bound ? this.secret : '';
  }

  /** Starts listening; a port already in use is reported, not fatal. */
  start(): Promise<void> {
    this.secret = ensureToken(this.deps);
    this.server = http.createServer((req, res) => void this.serve(req, res));
    return new Promise((resolve) => {
      this.server!.once('error', (e) => (console.warn('[oya] MCP port unavailable:', e.message), resolve()));
      this.server!.listen(C.MCP_PORT, C.MCP_HOST, () => ((this.bound = true), resolve()));
    });
  }

  /** Stops listening. */
  stop(): void {
    releaseAllViewports();
    this.server?.close();
    this.server = null;
    this.bound = false;
  }

  /** Answers one request; nothing it throws escapes into the main process. */
  private async serve(req: IncomingMessage, res: ServerResponse): Promise<void> {
    const reply = await this.reply(req).catch(() => ({ status: C.Status.BAD_REQUEST }) as Reply);
    res.writeHead(reply.status, { 'Content-Type': 'application/json', ...reply.headers });
    res.end(reply.body ? JSON.stringify(reply.body) : undefined);
  }

  /** Checks where the request comes from and that it carries the token, then answers it. */
  private async reply(req: IncomingMessage): Promise<Reply> {
    const refused = this.refusal(req);
    if (refused) return { status: refused };
    if (req.method === 'DELETE') return this.endSession(req);
    if (req.method !== 'POST') return { status: C.Status.METHOD_NOT_ALLOWED, headers: { Allow: 'POST, DELETE' } };
    return this.rpc(req);
  }

  /** Releases the session's lane and closes tabs it opened when the MCP client ends its session. */
  private endSession(req: IncomingMessage): Reply {
    const given = validSessionId(req);
    if (given) {
      releaseViewport(`mcp-${given}`);
      inLane(`mcp-${given}`, () => this.deps.agentTab?.release());
    }
    return { status: C.Status.OK };
  }

  /** The status that refuses this request, or 0 when it may go on. */
  private refusal(req: IncomingMessage): number {
    if (req.headers.origin !== undefined) return C.Status.FORBIDDEN; // a web page, never an agent
    if (!isLocalHost(req.headers.host)) return C.Status.FORBIDDEN; // DNS rebinding
    if (new URL(req.url || '/', 'http://local').pathname !== C.MCP_PATH) return C.Status.NOT_FOUND;
    return this.authorized(req.headers.authorization) ? 0 : C.Status.UNAUTHORIZED;
  }

  /** Whether the request carries this port's token. */
  private authorized(header: string | undefined): boolean {
    const given = Buffer.from(String(header ?? '').replace(/^Bearer\s+/i, ''));
    const expected = Buffer.from(this.secret);
    return given.length === expected.length && timingSafeEqual(given, expected);
  }

  /** Reads one JSON-RPC message and answers it as the session's agent. */
  private async rpc(req: IncomingMessage): Promise<Reply> {
    const body = await readMcpBody(req);
    if (body === null) return { status: C.Status.PAYLOAD_TOO_LARGE };
    const message = parse(body);
    const session = sessionOf(req, message);
    const result = await answer(message, (action, params) => this.run(session, action, params));
    const headers = { 'Mcp-Session-Id': session };
    return result ? { status: C.Status.OK, body: result, headers } : { status: C.Status.ACCEPTED, headers };
  }

  /** Runs one browser command as session `session`'s agent, after control lets automation in. */
  private async run(session: string, action: string, params: Record<string, unknown>): Promise<LocalResult> {
    const finish = await this.deps.control.beginLocalCommand().catch((e: Error) => e);
    if (finish instanceof Error)
      return { ok: false, data: null, error: `${finish.message}. Switch Oya to OyaAI Mode.` };
    const work = this.deps.commands.runLocal(`local-${randomUUID()}`, `mcp-${session}`, action, params);
    // Control is held until the command really ends, even when the agent stops waiting for it.
    void work.finally(finish);
    return withTimeout(work);
  }
}

/** Whether a Host header names this machine's loopback address. */
function isLocalHost(host: string | undefined): boolean {
  return host === `${C.MCP_HOST}:${C.MCP_PORT}` || host === `localhost:${C.MCP_PORT}`;
}

/** Valid Mcp-Session-Id header value, or empty string when missing or malformed. */
function validSessionId(req: IncomingMessage): string {
  const given = String(req.headers['mcp-session-id'] ?? '');
  return /^[A-Za-z0-9-]{1,64}$/.test(given) ? given : '';
}

/** The session a request belongs to: a fresh one on initialize, else the one its header names. */
function sessionOf(req: IncomingMessage, message: unknown): string {
  const given = validSessionId(req);
  const initializing = (message as Partial<Record<'method', unknown>> | null)?.method === 'initialize';
  if (!initializing && given) return given;
  return randomBytes(C.MCP_SESSION_BYTES).toString('hex');
}

/** The body as text, or null past the size limit. */
async function readMcpBody(req: IncomingMessage): Promise<string | null> {
  let size = 0;
  const parts: Buffer[] = [];
  for await (const chunk of req as AsyncIterable<Buffer>) {
    size += chunk.length;
    if (size > C.MCP_MAX_BODY_BYTES) return null;
    parts.push(chunk);
  }
  return Buffer.concat(parts).toString('utf8');
}

/** The body as JSON, or a marker the protocol refuses. */
function parse(body: string): unknown {
  try {
    return JSON.parse(body);
  } catch {
    return { parseError: true };
  }
}

/** A tool call that has not answered in time is reported, not waited on forever. */
function withTimeout(work: Promise<LocalResult>): Promise<LocalResult> {
  const late: LocalResult = { ok: false, data: null, error: 'The browser did not answer in time.' };
  return Promise.race([
    work,
    new Promise<LocalResult>((resolve) => setTimeout(() => resolve(late), C.MCP_CALL_TIMEOUT_MS).unref()),
  ]);
}
