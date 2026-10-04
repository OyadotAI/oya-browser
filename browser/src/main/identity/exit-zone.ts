/**
 * Where a proxied persona's traffic comes out, as a timezone. Sites compare the
 * timezone with where the IP is, and a residential exit moves from session to
 * session, so a persona's fixed zone matched its exit only by luck: Los Angeles
 * over Denver and New York IPs on the browserwars stealth bench. Asked once per
 * session, through that session's proxy, so the answer is the proxy's exit.
 *
 * net.request, not session.fetch: a proxy's auth challenge reaches the app's
 * `login` event only for a tab's requests (src/anonymity/proxy.ts), so a fetch with
 * no tab was refused at the tunnel (ERR_TUNNEL_CONNECTION_FAILED). The request's
 * own `login` event is answered here with the same credentials.
 */
import type { ClientRequest, IncomingMessage, Net, Session } from 'electron';
import { EXIT_GEO_URL, EXIT_GEO_TIMEOUT_MS } from './constants.ts';

/** The proxy credentials the exit's challenge is answered with. */
export interface ProxyCredentials {
  /** The proxy user, often carrying the sticky session id. */
  username?: string;
  /** Its password. */
  password?: string;
}

/** Settles a promise with one value. */
type Settle<T> = (value: T) => void;

/** Wraps a settle function so it clears the deadline first. */
type Deadline = <T>(fn: Settle<T>) => Settle<T>;

/** A proxy credential pair or nothing, as the persona carries it. */
type Proxy = ProxyCredentials | null | undefined;

/** What ExitZone needs from Electron. */
export interface ExitZoneDeps {
  /** Electron's net, whose requests can ride a session and answer its proxy. */
  net: Pick<Net, 'request'>;
}

/** The zone a lookup body names, or null; logs when it names none. */
function zoneOf(body: string): string | null {
  const zone = knownZone(JSON.parse(body)?.timezone);
  if (!zone) console.error('[anonymity] exit timezone lookup gave no zone; keeping the persona zone');
  return zone;
}

/** `zone` when this Chromium knows it as a timezone, otherwise null. */
const knownZone = (zone: unknown): string | null =>
  typeof zone === 'string' && Intl.supportedValuesOf('timeZone').includes(zone) ? zone : null;

/** A thrown value's message, or the value itself when it has none. */
const messageOrSelf = (err: unknown): unknown => (err as Error | null)?.message || err;

/** Aborts `req` and rejects once the deadline passes; wraps a settle function so it clears the deadline first. */
function withDeadline(req: ClientRequest, reject: Settle<Error>): Deadline {
  const timer = setTimeout(() => (req.abort(), reject(new Error('timed out'))), EXIT_GEO_TIMEOUT_MS);
  return (fn) => (value) => (clearTimeout(timer), fn(value));
}

/** Answers the request's proxy challenge with the persona's credentials, as the tabs' is answered. */
function answerAuth(req: ClientRequest, proxy: Proxy): void {
  req.on('login', (_authInfo, answer) => answer(proxy?.username, proxy?.password || ''));
}

/** Collects a response body as text. */
function readBody(res: IncomingMessage, resolve: Settle<string>, reject: Settle<Error>): void {
  const chunks: Buffer[] = [];
  res.on('data', (chunk) => chunks.push(chunk));
  res.on('end', () => resolve(Buffer.concat(chunks).toString('utf8')));
  res.on('error', reject);
}

/** Looks up proxied personas' exit timezones over Electron's net. */
export class ExitZone {
  /** Electron's net, whose requests can ride a session and answer its proxy. */
  private readonly net: Pick<Net, 'request'>;

  /** `net` is Electron's net module. */
  constructor(deps: ExitZoneDeps) {
    this.net = deps.net;
  }

  /** The exit's IANA timezone, asked through `ses` and its `proxy`; null when the lookup fails or answers anything else. */
  async timezone(ses: Session, proxy: Proxy, url: string = EXIT_GEO_URL): Promise<string | null> {
    try {
      return zoneOf(await this.getThrough(ses, proxy, url));
    } catch (err) {
      console.error('[anonymity] exit timezone lookup failed, keeping the persona zone:', messageOrSelf(err));
      return null;
    }
  }

  /** The body of a GET through the session, answering its proxy's challenge, within the deadline. */
  private getThrough(ses: Session, proxy: Proxy, url: string): Promise<string> {
    return new Promise((resolve, reject) => {
      const req = this.net.request({ url, session: ses, useSessionCookies: false });
      const done = withDeadline(req, reject);
      answerAuth(req, proxy);
      req.on('response', (res) => readBody(res, done(resolve), done(reject)));
      req.on('error', done(reject));
      req.end();
    });
  }
}
