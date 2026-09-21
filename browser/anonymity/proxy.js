/**
 * Proxy configuration, SOCKS5/HTTP/HTTPS proxy support for Electron sessions.
 */

const net = require('net');
const { DEFAULT_PORT, LOOPBACK } = require('./constants');

/** The proxy-auth `login` handler installed for each session, so a reconfigure can remove it. */
const authHandlers = new WeakMap();

/**
 * The server sends a proxy as { url, username, password }; older profiles and
 * governance send { host, port, type }. One shape from here on.
 */
function normalizeProxy(proxyConfig) {
  if (!proxyConfig?.url || proxyConfig.host) return proxyConfig;
  const u = new URL(proxyConfig.url);
  const type = u.protocol.replace(':', '');
  const port = Number(u.port) || (type === 'https' ? DEFAULT_PORT.https : DEFAULT_PORT.http);
  return { ...proxyConfig, type, host: u.hostname, port };
}

/**
 * Bytes through the operator's residential proxy, which is billed per GB.
 * Chromium talks to a local pass-through that pipes to the vendor gateway, so
 * every byte either way is counted, TLS and headers included, the same thing
 * the vendor bills. Proxy auth passes through untouched.
 */
let meteredBytes = 0;
/** One local pass-through per gateway (`host:port` → its local port). */
const meters = new Map();

/** Adds every byte the stream carries to the metered total. */
function countBytes(stream) {
  stream.on('data', (d) => (meteredBytes += d.length));
}

/** When either side errors or closes, both go. */
function closeTogether(client, upstream) {
  const end = () => {
    client.destroy();
    upstream.destroy();
  };
  client.on('error', end);
  upstream.on('error', end);
  client.on('close', end);
  upstream.on('close', end);
}

/** Pipes one Chromium connection to the gateway, counting every byte both ways. */
function pipeCounted(client, host, port) {
  const upstream = net.connect({ host, port });
  countBytes(client);
  countBytes(upstream);
  client.pipe(upstream).pipe(client);
  closeTogether(client, upstream);
}

/** Starts a loopback pass-through to the gateway; resolves to its local port. */
function listenLocal(host, port) {
  return new Promise((resolve, reject) => {
    const server = net.createServer((client) => pipeCounted(client, host, port));
    server.once('error', reject);
    server.listen(0, LOOPBACK, () => resolve(server.address().port));
  });
}

/** The local port that meters traffic to `host:port`, started once per gateway. */
function meter(host, port) {
  const key = `${host}:${port}`;
  if (!meters.has(key)) meters.set(key, listenLocal(host, port));
  return meters.get(key);
}

/** Bytes counted since the last call. */
function takeProxyBytes() {
  const n = meteredBytes;
  meteredBytes = 0;
  return n;
}

/** A metered proxy is reached through its local pass-through instead of directly. */
async function viaMeter(proxyConfig) {
  return { ...proxyConfig, type: 'http', host: LOOPBACK, port: await meter(proxyConfig.host, proxyConfig.port) };
}

/** The proxy rule Chromium takes for this config. */
function proxyUrl(proxyConfig) {
  const type = proxyConfig.type || 'http';
  // SOCKS5 automatically routes DNS through the proxy
  if (type === 'socks5' || type === 'socks') return `socks5://${proxyConfig.host}:${proxyConfig.port}`;
  return `${type === 'https' ? 'https' : 'http'}://${proxyConfig.host}:${proxyConfig.port}`;
}

/** Drops the session's previous proxy-auth handler, if any. */
function removeAuthHandler(app, ses) {
  const previous = authHandlers.get(ses);
  if (!previous) return;
  app.removeListener('login', previous);
  authHandlers.delete(ses);
}

/** Whether an auth challenge is this session's proxy asking, and nobody else. */
function isOurProxy(ses, proxyConfig, webContents, authInfo) {
  const sameHost = authInfo.host === proxyConfig.host && Number(authInfo.port) === Number(proxyConfig.port);
  return webContents?.session === ses && authInfo.isProxy && sameHost;
}

/** Answers this session's proxy-auth challenges with the configured credentials. */
function addAuthHandler(app, ses, proxyConfig) {
  const handler = (event, webContents, details, authInfo, callback) => {
    if (!isOurProxy(ses, proxyConfig, webContents, authInfo)) return;
    event.preventDefault();
    callback(proxyConfig.username, proxyConfig.password || '');
  };
  authHandlers.set(ses, handler);
  app.on('login', handler);
}

/** Points the session at the proxy (or direct when there is none) and answers its auth. */
async function applyRules(app, ses, proxyConfig) {
  if (!proxyConfig || !proxyConfig.host) {
    await ses.setProxy({ mode: 'direct' });
    return;
  }
  await ses.setProxy({ proxyRules: proxyUrl(proxyConfig), proxyBypassRules: '<-loopback>' });
  // Handle proxy authentication
  if (proxyConfig.username) addAuthHandler(app, ses, proxyConfig);
}

/**
 * Apply proxy settings to an Electron session.
 * @param {Electron.Session} ses
 * @param {{ type: string, host: string, port: number, username?: string, password?: string }} proxyConfig
 */
async function configureProxy(ses, proxyConfig) {
  proxyConfig = normalizeProxy(proxyConfig);
  if (proxyConfig?.metered && proxyConfig.host) proxyConfig = await viaMeter(proxyConfig);
  const { app } = require('electron');
  removeAuthHandler(app, ses);
  await applyRules(app, ses, proxyConfig);
}

/**
 * Apply DNS leak prevention flags.
 * Must be called before app.whenReady().
 * @param {Electron.App} app
 */
function applyDNSLeakPrevention(app) {
  // Disable DoH to prevent DNS bypass
  app.commandLine.appendSwitch('disable-features', 'DnsOverHttps');
  // Disable async DNS resolver that might bypass proxy
  app.commandLine.appendSwitch('disable-async-dns');
}

module.exports = { configureProxy, normalizeProxy, meter, takeProxyBytes, applyDNSLeakPrevention };
