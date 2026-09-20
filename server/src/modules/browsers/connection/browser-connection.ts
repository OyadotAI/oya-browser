/**
 * One Oya browser's control socket. It runs the connection's stages in order
 * (admission, persona, registration, welcome, heartbeat) and routes every later
 * message to its handler. Each stage lives in its own module.
 */
import { registry } from '../registry.ts';
import { destroyMcpServer } from '../../../mcp/server.ts';
import { metrics } from '../../../platform/metrics.ts';
import * as usage from '../../../platform/usage.ts';
import { container } from '../../../app/container.ts';
import { closeRelays } from '../cdp-relay.ts';
import { getAll as getAllCookies } from '../../personas/cookies.ts';
import { failPending } from './commands.ts';
import { HANDLERS, type Connection } from './handlers/index.ts';
import { admit, takePersona, Rejection } from './admission.ts';
import { adopt, list, providerOf, type Registration } from './registration.ts';
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
    if (this.authenticated) await HANDLERS[msg.type]?.(this, msg);
  }

  /** The control plane or the server refused: release the persona and close. */
  private fail(err) {
    if (this.persona) container.personas.release(this.persona, this.browserId);
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
    if (this.authStarted) return this.ws.close(CloseCode.REJECTED, 'Already authenticating');
    this.authStarted = true;
    clearTimeout(this.authTimer);
    if (!(await this.admit(msg))) return;
    const provider = providerOf(this.browserId, msg.provider);
    if (await this.register(msg, provider)) await this.welcome(provider);
  }

  /** Admits the browser and takes its persona; false if it was refused. */
  private async admit(msg) {
    try {
      Object.assign(this, await admit(msg));
      this.authenticated = true;
      this.persona = takePersona(this.apiKey, this.browserId, msg.persona);
      return true;
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

  /** Adopts and lists the session; false (persona released) if the socket closed meanwhile. */
  private async register(msg, provider: string) {
    const reg = this.registration(msg.api_key);
    await adopt(reg, provider, msg.enrollment_token);
    if (!this.isOpen()) {
      container.personas.release(this.persona, this.browserId);
      return false;
    }
    await list(reg, provider, msg);
    return true;
  }

  /** What registration needs, from this connection. */
  private registration(authToken: string): Registration {
    return { apiKey: this.apiKey, browserId: this.browserId, persona: this.persona, ws: this.ws, authToken };
  }

  /** Sends the identity and the cookie jar, then starts the heartbeat. */
  private async welcome(provider: string) {
    const { fingerprint, residential } = identityFor(this.apiKey, this.persona, provider);
    this.residentialProxy = residential;
    this.send(await welcomeMessage(this.apiKey, this.browserId, this.persona, fingerprint));
    // The jar again as a sync, so the browser applies it at once.
    const cookies = getAllCookies(this.persona.id);
    if (cookies.length > 0) this.send({ type: 'cookie_sync', cookies });
    this.heartbeat.start();
  }

  /** Releases what the connection held, unless a reconnect already replaced it. */
  private onClose() {
    for (const finish of this.localCommands.values()) void finish().catch(() => {});
    this.localCommands.clear();
    clearTimeout(this.authTimer);
    this.heartbeat.stop();
    // Removing the entry of the connection that replaced us would make the browser flap on/off.
    if (this.browserId && this.isCurrent()) this.unregister();
  }

  /** Takes the browser out of every registry and returns its persona slot. */
  private unregister() {
    failPending(this.browserId, 'disconnected', 'Browser disconnected');
    closeRelays(this.browserId);
    registry.remove(this.browserId);
    destroyMcpServer(this.browserId);
    usage.browserDisconnected(this.apiKey, this.browserId);
    container.personas.release(this.persona, this.browserId);
    metrics.wsDisconnections.inc({ client: 'oya' });
    metrics.browsersConnected.set({}, registry.browsers.size);
  }
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
