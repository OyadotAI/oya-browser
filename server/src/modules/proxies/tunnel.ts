/**
 * GET `target` through an http(s) proxy with the standard library: CONNECT to
 * the target, then TLS inside the tunnel when the target is https. (This used
 * to import undici, which the server does not depend on, so every check
 * failed and put a working proxy into cooldown.)
 */
import http from 'http';
import https from 'https';
import tls from 'tls';
import { CHECK_TIMEOUT_MS, HTTP_OK, HTTP_PORT, HTTPS_PORT } from './constants.ts';

/** Fetches `target` through the proxy and resolves with the body; rejects on any failure or timeout. */
export function getVia(proxyUrl, username, password, target): Promise<string> {
  const auth = username
    ? { 'Proxy-Authorization': `Basic ${Buffer.from(`${username}:${password || ''}`).toString('base64')}` }
    : {};
  return new Promise((resolve, reject) => new TunnelledGet(proxyUrl, target, auth, resolve, reject).start());
}

/** The port a URL names, or its scheme's default. */
const portOf = (url) => Number(url.port) || (url.protocol === 'https:' ? HTTPS_PORT : HTTP_PORT);

/** One check request: the CONNECT, the GET inside it, and the timeout over both. */
class TunnelledGet {
  /** The proxy to tunnel through. */
  declare private readonly proxyUrl: URL;
  /** What to fetch. */
  declare private readonly target: URL;
  /** Proxy-Authorization header, when the proxy has credentials. */
  declare private readonly auth: object;
  /** Settles the check with the body. */
  declare private readonly resolve: (body: string) => void;
  /** Settles the check with an error. */
  declare private readonly reject: (err: Error) => void;
  /** The CONNECT request. */
  declare private req: any;
  /** The tunnelled socket, once the proxy answers. */
  declare private tunnel: any;
  /** Gives up after CHECK_TIMEOUT_MS. */
  declare private timer: ReturnType<typeof setTimeout>;

  /** Holds what the request needs; nothing is sent until start(). */
  constructor(proxyUrl, target, auth, resolve, reject) {
    this.proxyUrl = proxyUrl;
    this.target = target;
    this.auth = auth;
    this.resolve = resolve;
    this.reject = reject;
  }

  /** Sends the CONNECT and arms the timeout. */
  start() {
    this.req = this.openConnect();
    this.timer = setTimeout(() => this.timeout(), CHECK_TIMEOUT_MS);
    this.req.on('error', (e) => this.finish(e));
    this.req.on('connect', (res, socket) => this.onConnect(res, socket));
    this.req.end();
  }

  /** The CONNECT request to the proxy, for the target's host and port. */
  private openConnect() {
    const hostPort = `${this.target.hostname}:${portOf(this.target)}`;
    return (this.proxyUrl.protocol === 'https:' ? https : http).request({
      host: this.proxyUrl.hostname,
      port: portOf(this.proxyUrl),
      method: 'CONNECT',
      path: hostPort,
      headers: { host: hostPort, ...this.auth },
    });
  }

  /** Took too long: tear everything down and fail. */
  private timeout() {
    this.req.destroy();
    this.tunnel?.destroy();
    this.reject(new Error('proxy check timed out'));
  }

  /** Settles once, closing the tunnel either way. */
  private finish(err, body?) {
    clearTimeout(this.timer);
    this.tunnel?.destroy();
    if (err) this.reject(err);
    else this.resolve(body);
  }

  /** The proxy answered the CONNECT: on success, send the GET through the tunnel. */
  private onConnect(res, socket) {
    this.tunnel = socket;
    if (res.statusCode !== HTTP_OK) return this.finish(new Error(`proxy answered ${res.statusCode}`));
    const secure = this.target.protocol === 'https:';
    const get = (secure ? https : http).request(this.getOptions(socket, secure), (r) => this.readBody(r));
    get.on('error', (e) => this.finish(e));
    get.end();
  }

  /** The GET, over the tunnel (inside TLS when the target is https). */
  private getOptions(socket, secure) {
    return {
      ...this.where(),
      method: 'GET',
      agent: false,
      headers: { host: this.target.host, connection: 'close' },
      createConnection: () => (secure ? tls.connect({ socket, servername: this.target.hostname }) : socket),
    };
  }

  /** Host, port and path of the target. */
  private where() {
    return {
      host: this.target.hostname,
      port: portOf(this.target),
      path: this.target.pathname + this.target.search,
    };
  }

  /** Collects the answer; anything but 200 fails the check. */
  private readBody(r) {
    let data = '';
    r.setEncoding('utf8');
    r.on('data', (c) => {
      data += c;
    });
    r.on('end', () =>
      r.statusCode === HTTP_OK ? this.finish(null, data) : this.finish(new Error(`check returned ${r.statusCode}`)),
    );
  }
}
