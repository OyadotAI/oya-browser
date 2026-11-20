/**
 * One Oya browser's control socket. It runs the connection's stages in order
 * (admission, persona, registration, welcome, heartbeat) and routes every later
 * message to its handler. Each stage lives in its own module.
 */
import { registry } from '../registry.ts';
import { metrics } from '../../../platform/metrics.ts';
import { container } from '../../../app/container.ts';
import { getAll as getAllCookies } from '../../personas/cookies.ts';
import { releaseBrowser } from './release.ts';
import { HANDLERS, type Connection } from './handlers/index.ts';
import { admit, claimRegistered, hijack, takePersona, Rejection, type Admitted } from './admission.ts';
import { adopt, enlist, list, providerOf, type Registration } from './registration.ts';
import { identityFor, welcomeMessage } from './identity.ts';
import { Heartbeat } from './heartbeat.ts';
import { AUTH_DEADLINE_MS, CloseCode } from './constants.ts';

/** One browser socket, from its first message to its close. */
export class BrowserConnection implements Connection {
  /** The socket. */
  declare readonly ws: any;
  /** Remote address, so a rejection can be diagnosed from the server side. */
  declare readonly from: string;
  /** Pings and pong-timeout. */
  declare private readonly heartbeat: Heartbeat;
  /** Closes the socket if `auth` never arrives. */
  declare private readonly authTimer: ReturnType<typeof setTimeout>;
  /** Set once admitted. */
  browserId: string | null = null;
  /** The key the browser registered under. */
  apiKey: string | null = null;
  /** The persona it runs as; its concurrency slot is held while connected. */
  persona: any = null;
  /** Whether it uses the operator's metered residential gateway. */
  residentialProxy = false;
  /** A desktop control handoff is in progress. */
  changingControl = false;
  /** Desktop-held command slots, by token. */
  readonly localCommands = new Map<string, () => Promise<void>>();
  /** `auth` has arrived (a second one is refused). */
  private authStarted = false;
  /** Admission succeeded; other messages are handled from now on. */
  private authenticated = false;

  /** Takes over a freshly accepted socket. */
  constructor(ws, req) {
    this.ws = ws;
    this.from = req?.socket?.remoteAddress || 'unknown';
    this.heartbeat = new Heartbeat(ws, () => this.browserId);
    this.authTimer = setTimeout(() => this.authTimedOut(), AUTH_DEADLINE_MS);
    ws.on('message', (raw) => this.onMessage(raw));
    ws.on('close', () => this.onClose());
    ws.on('error', () => {}); // close follows
  }

  /** Whether this socket is still the one registered for its browser (a reconnect replaces it). */
  isCurrent() {
    return registry.get(this.browserId)?.ws === this.ws;
  }

  /** Whether the socket can still be written to. */
  isOpen() {
    return this.ws.readyState === this.ws.OPEN;
  }

  /** Sends a message; a socket that just went away is not an error. */
  send(message: object) {
    try {
      this.ws.send(JSON.stringify(message));
    } catch {}
  }

  /** Any sign of life keeps the heartbeat and the registry's last-seen current. */
  heard() {
    this.heartbeat.heard();
    registry.updateLastSeen(this.browserId);
  }

  /** Hands a message to authentication or to its handler; a failure closes the socket. */
  private async onMessage(raw) {
    const msg = parse(raw);
    if (msg) await this.route(msg).catch((err) => this.fail(err));
  }

  /** `auth` authenticates; everything else needs an authenticated socket. */
  private async route(msg) {
    if (msg.type === 'auth') return this.authenticate(msg);
    if (this.authenticated && Object.hasOwn(HANDLERS, msg.type)) await HANDLERS[msg.type](this, msg);
  }

  /** The control plane or the server refused: release held resources and close. */
  private fail(err) {
    releaseHeld(this);
    console.error('[ws] registration/message rejected:', err.message);
    this.ws.close(CloseCode.CONTROL_REJECTED, 'Control plane rejected connection');
  }

  /** Closes a socket that never authenticated. */
  private authTimedOut() {
    if (this.authenticated) return;
    console.warn(`[ws] ✗ ${this.from} rejected: no auth message within 10s`);
    this.ws.close(CloseCode.AUTH_TIMEOUT, 'Auth timeout');
  }

  /** The `auth` message: admission, then registration and the welcome. */
  private async authenticate(msg) {
    if (this.authStarted) return rejectDuplicateAuth(this);
    this.authStarted = true;
    clearTimeout(this.authTimer);
    if (!(await this.admit(msg))) return;
    const provider = providerOf(this.browserId, msg.provider);
    if (await this.register(msg, provider)) await this.welcome(provider);
  }

  /** Admits the browser and takes its persona; false if it was refused or hung up. */
  private async admit(msg) {
    try {
      return await acquireAdmission(this, msg);
    } catch (err) {
      return this.reject(err);
    }
  }

  /** A refusal closes the socket with its code; anything else is a failure. */
  private reject(err): false {
    if (!(err instanceof Rejection)) throw err;
    console.warn(`[ws] ✗ ${this.from} rejected: ${err.detail}`);
    metrics.wsConnections.inc({ outcome: err.outcome });
    this.ws.close(err.code, err.message);
    return false;
  }

  /** Adopts and lists the session; false if the socket closed or was replaced meanwhile. */
  private async register(msg, provider: string) {
    const reg = this.registration(msg.api_key);
    await adopt(reg, provider, msg.enrollment_token);
    if (!stillLive(this)) return false;
    enlist(reg, provider, msg);
    unclaimed.delete(this.browserId);
    await list(reg, provider, msg);
    return stillLive(this);
  }

  /** What registration needs, from this connection. */
  private registration(authToken: string): Registration {
    return { apiKey: this.apiKey, browserId: this.browserId, persona: this.persona, ws: this.ws, authToken };
  }

  /** Sends the identity and the cookie jar, then opens message routing and the heartbeat. */
  private async welcome(provider: string) {
    const { fingerprint, residential } = identityFor(this.apiKey, this.persona, provider);
    this.residentialProxy = residential;
    const greeting = await welcomeMessage(this.apiKey, this.browserId, this.persona, fingerprint);
    if (!stillLive(this)) return;
    sendGreeting(this, greeting);
    this.authenticated = true;
    this.heartbeat.start();
  }

  /** Releases what the connection held, unless a reconnect already replaced it. */
  private onClose() {
    for (const finish of this.localCommands.values()) void finish().catch(() => {});
    this.localCommands.clear();
    clearTimeout(this.authTimer);
    this.heartbeat.stop();
    releaseHeld(this);
  }
}

/** Connections holding a persona slot that has not yet been handed to `registry`, keyed by `browserId`. */
const unclaimed = new Map<string, BrowserConnection>();

/** Resolves the browser's key and persona slot while the socket stays open. */
async function acquireAdmission(conn: BrowserConnection, msg) {
  const admitted = await admit(msg);
  if (!conn.isOpen()) return false;
  claimSlot(conn, admitted, msg);
  return true;
}

/** Replaces any prior socket for `browserId`, takes the persona slot, and records the pre-registry claim. */
function claimSlot(conn: BrowserConnection, admitted: Admitted, msg) {
  claimRegistered(admitted.browserId, admitted.apiKey);
  clearPending(admitted.browserId, admitted.apiKey);
  Object.assign(conn, admitted);
  conn.persona = takePersona(conn.apiKey, conn.browserId, msg.persona, msg.host_platform);
  unclaimed.set(conn.browserId, conn);
}

/** Refuses another tenant's pending handshake, or releases and replaces an earlier one for the same key. */
function clearPending(browserId: string, apiKey: string) {
  const prior = unclaimed.get(browserId);
  if (!prior) return;
  if (prior.apiKey !== apiKey) throw hijack(browserId);
  unclaimed.delete(browserId);
  if (prior.persona) container.personas.release(prior.persona, browserId);
  closeReplaced(prior);
}

/** Closes a socket superseded during admission; a socket that already closed is not an error. */
function closeReplaced(conn: BrowserConnection) {
  try {
    conn.ws.close(CloseCode.REPLACED, 'Replaced by new connection');
  } catch {}
}

/** Sends `auth_ok` and, when non-empty, the persona's initial cookie jar. */
function sendGreeting(conn: BrowserConnection, greeting: object) {
  conn.send(greeting);
  const cookies = getAllCookies(conn.persona.id);
  if (cookies.length > 0) conn.send({ type: 'cookie_sync', cookies });
}

/** Refuses a duplicate `auth` frame, releasing any resources the socket already held. */
function rejectDuplicateAuth(conn: BrowserConnection) {
  releaseHeld(conn);
  conn.ws.close(CloseCode.REJECTED, 'Already authenticating');
}

/** Whether `conn` currently owns the pre-registry admission claim for its `browserId`. */
function isPending(conn: BrowserConnection) {
  return conn.browserId !== null && unclaimed.get(conn.browserId) === conn;
}

/** Whether `conn` is still open and active; releases held resources if it began closing. */
function stillLive(conn: BrowserConnection) {
  if (!conn.isOpen()) return (releaseHeld(conn), false);
  return isPending(conn) || conn.isCurrent();
}

/** Releases a connection's registration or pre-registry persona slot at most once. */
function releaseHeld(conn: BrowserConnection) {
  const preRegistry = isPending(conn);
  if (preRegistry && conn.browserId) unclaimed.delete(conn.browserId);
  if (conn.browserId && conn.isCurrent()) releaseRegistered(conn.browserId);
  else if (preRegistry && conn.persona) container.personas.release(conn.persona, conn.browserId);
}

/** Unregisters a live connection and records its disconnect metric. */
function releaseRegistered(browserId: string) {
  releaseBrowser(browserId, registry.get(browserId), 'disconnected');
  metrics.wsDisconnections.inc({ client: 'oya' });
}

/** A message as JSON with a type, or null. */
function parse(raw) {
  try {
    const msg = JSON.parse(raw.toString());
    return msg?.type ? msg : null;
  } catch {
    return null;
  }
}
