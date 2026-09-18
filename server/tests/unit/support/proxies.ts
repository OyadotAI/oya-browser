/**
 * Test doubles for the proxy pool: a CONNECT proxy and a target web server,
 * both on loopback in this process, so a health check runs end to end with
 * no outside network.
 */
import { createServer, type Server } from 'node:http';
import { connect } from 'node:net';
import type { AddressInfo } from 'node:net';

/** How the fake proxy answers a CONNECT. */
export type ProxyMode = 'tunnel' | 'refuse' | 'silent';

/** Listens on a loopback port and resolves with the server and its port. */
async function listen(server: Server) {
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  return (server.address() as AddressInfo).port;
}

/** A target that answers GET with `body` and `status`, recording the paths asked for. */
export async function targetServer(body = '{"ip":"203.0.113.9"}', status = 200) {
  const paths: string[] = [];
  const server = createServer((req, res) => {
    paths.push(req.url || '');
    res.writeHead(status, { 'content-type': 'application/json' }).end(body);
  });
  const port = await listen(server);
  return { port, paths, close: () => closeAll(server) };
}

/**
 * A CONNECT proxy: 'tunnel' pipes to the requested host, 'refuse' answers 407,
 * 'silent' never answers. Records the Proxy-Authorization headers it saw.
 */
export async function connectProxy(mode: ProxyMode = 'tunnel') {
  const auth: (string | undefined)[] = [];
  const server = createServer();
  const tunnels = new Set<{ destroy(): void }>();
  server.on('connect', (req, client) => {
    tunnels.add(client);
    auth.push(req.headers['proxy-authorization'] as string | undefined);
    if (mode === 'silent') return;
    if (mode === 'refuse') return client.end('HTTP/1.1 407 Proxy Authentication Required\r\n\r\n');
    const [host, port] = String(req.url).split(':');
    const upstream = connect(Number(port), host, () => {
      tunnels.add(upstream);
      client.write('HTTP/1.1 200 Connection Established\r\n\r\n');
      upstream.pipe(client).pipe(upstream);
    });
    upstream.on('error', () => client.destroy());
    client.on('error', () => upstream.destroy());
  });
  const port = await listen(server);
  // CONNECT sockets leave the server's books once upgraded, so they are closed by hand.
  const close = () => (tunnels.forEach((s) => s.destroy()), closeAll(server));
  return { port, auth, close };
}

/** Closes a server and every connection still open on it. */
function closeAll(server: Server) {
  server.closeAllConnections();
  return new Promise<void>((resolve) => server.close(() => resolve()));
}
