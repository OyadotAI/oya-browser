/** Callback admission is tested with an in-memory HTTP server: no port or external network is used. */
import { afterEach, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import { EventEmitter } from 'node:events';
import { desktopCallback } from '../../../../src/main/connection/desktop-callback.ts';

/** An HTTP seam records lifecycle and dispatches requests synchronously. */
function serverFake() {
  const server: any = new EventEmitter();
  server.closed = 0;
  server.listen = (_port, _host, ready) => queueMicrotask(ready);
  server.address = () => ({ port: 41777 });
  server.close = () => server.closed++;
  server.closeAllConnections = () => {};
  server.closeIdleConnections = () => {};
  mock.method(http, 'createServer', () => server);
  return server;
}
/** Drive the public request handler without contacting even a loopback socket. */
function request(server, url, method = 'GET', host = '127.0.0.1:41777') {
  const response: any = {
    status: 0,
    body: '',
    writeHead(status, headers) {
      this.status = status;
      this.headers = headers;
      return this;
    },
    end(body) {
      this.body = body;
    },
  };
  server.emit('request', { method, url, headers: { host } }, response);
  return response;
}
afterEach(() => mock.restoreAll());
it('rejects forged callbacks without consuming the genuine one-use return', async () => {
  const server = serverFake(),
    abort = new AbortController();
  const listener = await desktopCallback('native-secret', abort.signal);
  for (const [url, method, host] of [
    ['//[', 'GET', '127.0.0.1:41777'],
    ['http://[', 'GET', '127.0.0.1:41777'],
    ['/oya-desktop/callback?code=one&state=wrong', 'GET', '127.0.0.1:41777'],
    ['/oya-desktop/callback?code=one&state=native-secret', 'POST', '127.0.0.1:41777'],
    ['/oya-desktop/callback?code=one&state=native-secret', 'GET', 'evil.test:41777'],
    ['/other?code=one&state=native-secret', 'GET', '127.0.0.1:41777'],
    ['/oya-desktop/callback?code=one&state=native-secret&server=wss://evil.test', 'GET', '127.0.0.1:41777'],
    ['/oya-desktop/callback?code=one&code=two&state=native-secret', 'GET', '127.0.0.1:41777'],
  ])
    assert.equal(request(server, url, method, host).status, 400);
  const accepted = request(server, '/oya-desktop/callback?code=one&state=native-secret');
  assert.equal(accepted.status, 200);
  assert.equal(await listener.done, 'one');
  assert.equal(request(server, '/oya-desktop/callback?code=two&state=native-secret').status, 400);
  assert(!accepted.body.includes('native-secret'));
  assert(!accepted.body.includes('code=one'));
  assert.equal(accepted.headers['Cache-Control'], 'no-store');
  listener.close();
});
it('cancellation closes the listener and settles its pending result', async () => {
  const server = serverFake(),
    abort = new AbortController();
  const listener = await desktopCallback('secret', abort.signal);
  const rejected = assert.rejects(listener.done, /didn’t finish/);
  abort.abort();
  await rejected;
  assert(server.closed > 0);
  assert.equal(request(server, '/oya-desktop/callback?code=one&state=secret').status, 400);
});
