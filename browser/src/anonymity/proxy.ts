/**
 * Proxy configuration, SOCKS5/HTTP/HTTPS proxy support for Electron sessions.
 */
import net from 'node:net';
import type { App, AuthInfo, Event, Session, WebContents } from 'electron';
import { DEFAULT_PORT, LOOPBACK } from './constants.ts';

/** A proxy as the server sends it ({ url, username, password }) or as older profiles and governance do ({ host, port, type }). */
export interface ProxyConfig {
  /** The proxy URL, in the server's shape. */
  url?: string;
  /** http, https, socks or socks5. */
  type?: string;
  /** The proxy's host. */
  host?: string;
  /** The proxy's port. */
  port?: number;
  /** Proxy-auth user name. */
  username?: string;
  /** Proxy-auth password. */
  password?: string;
  /** Count the bytes through it (a residential proxy billed per GB). */
  metered?: boolean;
}

/** The proxy-auth `login` listener installed for a session. */
type LoginHandler = (
  event: Event,
  webContents: WebContents | null,
  details: unknown,
  authInfo: AuthInfo,
  callback: (username?: string, password?: string) => void,
) => void;

/** The parts of Electron's app the proxy uses: its `login` event. */
export type LoginApp = Pick<App, 'on' | 'removeListener'>;

/** The proxy-auth `login` handler installed for each session, so a reconfigure can remove it. */
const authHandlers = new WeakMap<object, LoginHandler>();

/**
 * Bytes through the operator's residential proxy, which is billed per GB.
 * Chromium talks to a local pass-through that pipes to the vendor gateway, so
 * every byte either way is counted, TLS and headers included, the same thing
 * the vendor bills. Proxy auth passes through untouched.
 */
const metered = { bytes: 0 };
/** One local pass-through per gateway (`host:port` → its local port). */
const meters = new Map<string, Promise<number>>();

/**
 * The server sends a proxy as { url, username, password }; older profiles and
 * governance send { host, port, type }. One shape from here on.
 */
export function normalizeProxy<T extends ProxyConfig | null | undefined>(proxyConfig: T): T {
  if (!proxyConfig?.url || proxyConfig.host) return proxyConfig;
  const u = new URL(proxyConfig.url);
  const type = u.protocol.replace(':', '');
  const port = Number(u.port) || (type === 'https' ? DEFAULT_PORT.https : DEFAULT_PORT.http);
  return { ...proxyConfig, type, host: u.hostname, port } as T;
}

/** Adds every byte the stream carries to the metered total. */
function countBytes(stream: net.Socket): void {
  stream.on('data', (d: Buffer) => (metered.bytes += d.length));
}

/** When either side errors or closes, both go. */
function closeTogether(client: net.Socket, upstream: net.Socket): void {
  const end = () => {
    client.destroy();
    upstream.destroy();
  };
  for (const side of [client, upstream]) side.on('error', end).on('close', end);
}

/** Pipes one Chromium connection to the gateway, counting every byte both ways. */
function pipeCounted(client: net.Socket, host: string, port: number): void {
  const upstream = net.connect({ host, port });
  countBytes(client);
  countBytes(upstream);
  client.pipe(upstream).pipe(client);
  closeTogether(client, upstream);
}

/** Starts a loopback pass-through to the gateway; resolves to its local port. */
function listenLocal(host: string, port: number): Promise<number> {
  return new Promise((resolve, reject) => {
    const server = net.createServer((client) => pipeCounted(client, host, port));
    server.once('error', reject);
    server.listen(0, LOOPBACK, () => resolve((server.address() as net.AddressInfo).port));
  });
}

/** The local port that meters traffic to `host:port`, started once per gateway. */
export function meter(host: string, port: number): Promise<number> {
  const key = `${host}:${port}`;
  if (!meters.has(key)) meters.set(key, listenLocal(host, port));
  return meters.get(key)!;
}

/** Bytes counted since the last call. */
export function takeProxyBytes(): number {
  const n = metered.bytes;
  metered.bytes = 0;
  return n;
}

/** A metered proxy is reached through its local pass-through instead of directly. */
async function viaMeter(proxyConfig: ProxyConfig & Required<Pick<ProxyConfig, 'host'>>): Promise<ProxyConfig> {
  const port = await meter(proxyConfig.host, Number(proxyConfig.port));
  return { ...proxyConfig, type: 'http', host: LOOPBACK, port };
}

/** The proxy rule Chromium takes for this config. */
function proxyUrl(proxyConfig: ProxyConfig): string {
  const type = proxyConfig.type || 'http';
  // SOCKS5 automatically routes DNS through the proxy
  if (type === 'socks5' || type === 'socks') return `socks5://${proxyConfig.host}:${proxyConfig.port}`;
  return `${type === 'https' ? 'https' : 'http'}://${proxyConfig.host}:${proxyConfig.port}`;
}

/** Drops the session's previous proxy-auth handler, if any. */
function removeAuthHandler(app: LoginApp, ses: Session): void {
  const previous = authHandlers.get(ses);
  if (!previous) return;
  app.removeListener('login', previous);
  authHandlers.delete(ses);
}

/** Whether an auth challenge is this session's proxy asking, and nobody else. */
function isOurProxy(ses: Session, proxyConfig: ProxyConfig, webContents: WebContents | null, authInfo: AuthInfo) {
  const sameHost = authInfo.host === proxyConfig.host && Number(authInfo.port) === Number(proxyConfig.port);
  return webContents?.session === ses && authInfo.isProxy && sameHost;
}

/** Answers this session's proxy-auth challenges with the configured credentials. */
function addAuthHandler(app: LoginApp, ses: Session, proxyConfig: ProxyConfig): void {
  const handler: LoginHandler = (event, webContents, _details, authInfo, callback) => {
    if (!isOurProxy(ses, proxyConfig, webContents, authInfo)) return;
    event.preventDefault();
    callback(proxyConfig.username, proxyConfig.password || '');
  };
  authHandlers.set(ses, handler);
  app.on('login', handler);
}

/** Points the session at the proxy (or direct when there is none) and answers its auth. */
async function applyRules(app: LoginApp, ses: Session, proxyConfig: ProxyConfig | null | undefined): Promise<void> {
  if (!proxyConfig || !proxyConfig.host) {
    await ses.setProxy({ mode: 'direct' });
    return;
  }
  await ses.setProxy({ proxyRules: proxyUrl(proxyConfig), proxyBypassRules: '<-loopback>' });
  // Handle proxy authentication
  if (proxyConfig.username) addAuthHandler(app, ses, proxyConfig);
}

/** Apply proxy settings to an Electron session; `app` carries the proxy-auth `login` event. */
export async function configureProxy(
  ses: Session,
  proxyConfig: ProxyConfig | null | undefined,
  app: LoginApp,
): Promise<void> {
  let config = normalizeProxy(proxyConfig);
  if (config?.metered && config.host) config = await viaMeter({ ...config, host: config.host });
  removeAuthHandler(app, ses);
  await applyRules(app, ses, config);
}

/**
 * Apply DNS leak prevention flags.
 * Must be called before app.whenReady().
 */
export function applyDNSLeakPrevention(app: Pick<App, 'commandLine'>): void {
  // Disable DoH to prevent DNS bypass. appendSwitch replaces a switch's value, and this
  // runs after the telemetry flags: it used to wipe their whole --disable-features list.
  const already = app.commandLine.getSwitchValue?.('disable-features') || '';
  app.commandLine.appendSwitch('disable-features', [...already.split(',').filter(Boolean), 'DnsOverHttps'].join(','));
  // Disable async DNS resolver that might bypass proxy
  app.commandLine.appendSwitch('disable-async-dns');
}
