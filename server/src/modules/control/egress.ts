/** Authenticated managed-browser egress. Public DNS is resolved once and pinned for the connection. */
import http from 'node:http';
import net from 'node:net';
import { control, hash, terminal } from './service.ts';
import { assertSafeTarget, isPrivateAddress } from '../../platform/net-guard.ts';
/**
 * Host rules: an exact lowercase name, `*.domain` (any subdomain, never the bare
 * domain), or a `*` inside a label (`*-aiplatform.googleapis.com`). A wildcard is
 * `[^.]*`, so it never crosses a label boundary, and the `*.` prefix stays a raw
 * suffix test — tightening it to a DNS-label class would stop matching real hosts
 * like `_dmarc.example.com`, which reads as hardening but is a humanHosts bypass.
 * Star count and rule length are capped by validatePolicy; without that cap these
 * patterns backtrack catastrophically.
 * Kept byte-identical to browser/governance.js — the proxy and the renderer
 * disagreeing means a host one allows and the other blocks.
 */
const patterns = new Map();
/** The cached RegExp for one host rule. */
function compile(rule) {
  let pattern = patterns.get(rule);
  if (!pattern) {
    const subdomain = rule.startsWith('*.');
    const body = (subdomain ? rule.slice(2) : rule)
      .replace(/[.+?^${}()|[\]\\]/g, '\\$&')
      .replace(/\*/g, '[^.]*');
    pattern = new RegExp(`^${subdomain ? '(?:[^.]*\\.)+' : ''}${body}$`);
    // ponytail: clear-on-full, since rules are tenant-settable; LRU if it ever churns.
    if (patterns.size > 500) patterns.clear();
    patterns.set(rule, pattern);
  }
  return pattern;
}
/** Whether `host` matches any of the rules above (case- and trailing-dot-insensitive). */
export function allowedHost(host, rules) {
  const normalized = host.toLowerCase().replace(/\.$/, '');
  return normalized.length <= 253 && rules.some(rule => compile(rule).test(normalized));
}
/**
 * Authenticate the proxy request against its managed session and resolve where it may go:
 * HTTP(S) on 80/443 only, within the session's host policies, never a private address.
 */
async function destination(req, raw) {
  const auth = req.headers['proxy-authorization'];
  if (!auth?.startsWith('Basic ')) throw new Error('Proxy credentials required');
  const decoded = Buffer.from(auth.slice(6), 'base64').toString(), colon = decoded.indexOf(':');
  const id = decoded.slice(0, colon), token = decoded.slice(colon + 1);
  const session = await control().store.get('session', id);
  if (!session?.managed || session.egressHash !== hash(token) || terminal.has(session.state) || ['cleanup_pending', 'stopping'].includes(session.state)) throw new Error('Session unavailable');
  const url = new URL(raw), port = Number(url.port || (url.protocol === 'https:' ? 443 : 80));
  if (!['http:', 'https:'].includes(url.protocol) || ![80, 443].includes(port)) throw new Error('Only HTTP(S) egress on standard ports is allowed');
  const human = session.control?.mode === 'human' && session.control.expiresAt > Date.now();
  for (const policy of session.policies || []) {
    if (policy.allowedHosts && !allowedHost(url.hostname, policy.allowedHosts)) throw new Error('Destination denied');
    // Browser hooks enforce this too; the proxy is the boundary a compromised renderer cannot skip.
    if (policy.humanHosts && allowedHost(url.hostname, policy.humanHosts) && !human) throw new Error('Destination requires human control');
  }
  const target = await assertSafeTarget(url.href, { protocols: ['http:', 'https:'], label: 'Egress target' });
  if (target.addresses.some(isPrivateAddress)) throw new Error('Private destinations are forbidden for governed sessions');
  return { url, port, address: target.addresses[0] };
}
/** The forward proxy: plain HTTP is relayed, HTTPS goes through CONNECT to the pinned address. */
export function createEgressServer() {
  const server = http.createServer(async (req, res) => {
    if (!req.headers['proxy-authorization']) { res.writeHead(407, { 'Proxy-Authenticate': 'Basic realm="Oya managed egress"' }); res.end(); return; }
    try {
      const { url, address, port } = await destination(req, req.url);
      if (url.protocol !== 'http:') throw new Error('Use CONNECT for HTTPS');
      const headers = { ...req.headers, host: url.host };
      delete headers['proxy-authorization']; delete headers['proxy-connection'];
      const upstream = http.request({ host: address, port, method: req.method, path: url.pathname + url.search, headers }, incoming => {
        res.writeHead(incoming.statusCode, incoming.headers); incoming.pipe(res);
      });
      upstream.on('error', () => { if (!res.headersSent) res.writeHead(502); res.end(); });
      upstream.setTimeout(30000, () => upstream.destroy());
      req.on('aborted', () => upstream.destroy()); req.pipe(upstream);
    } catch { res.writeHead(403); res.end('Egress denied'); }
  });
  server.on('connect', async (req, socket, head) => {
    if (!req.headers['proxy-authorization']) { socket.end('HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: Basic realm="Oya managed egress"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n'); return; }
    try {
      const { address, url } = await destination(req, `https://${req.url}`);
      if (Number(url.port || 443) !== 443) throw new Error('CONNECT requires port 443');
      const upstream = net.connect({ host: address, port: 443 });
      upstream.once('connect', () => {
        socket.write('HTTP/1.1 200 Connection Established\r\n\r\n');
        if (head.length) upstream.write(head);
        socket.pipe(upstream); upstream.pipe(socket);
      });
      upstream.on('error', () => socket.destroy()); socket.on('error', () => upstream.destroy());
      socket.on('close', () => upstream.destroy()); upstream.on('close', () => socket.destroy());
      upstream.setTimeout(60000, () => upstream.destroy());
    } catch { socket.end('HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n'); }
  });
  return server;
}
