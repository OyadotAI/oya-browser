/**
 * Unit tests for anonymity/proxy.js: proxy shapes, session configuration and
 * auth through a fake Electron, and the loopback byte meter.
 */
const { describe, it, before, after, mock } = require('node:test');
const assert = require('node:assert/strict');
const net = require('node:net');
const { EventEmitter } = require('node:events');
const { installElectron, freshRequire } = require('../support/fakes.cjs');

/** A session that records every setProxy call. */
const fakeSession = () => ({ proxies: [], setProxy: async (value) => fakeSessionLast(value) });
let lastProxy;
/** Keeps the last proxy config set on any fake session. */
const fakeSessionLast = (value) => (lastProxy = value);

describe('proxy', () => {
  const app = new EventEmitter();
  let restore;
  let proxy;
  before(() => {
    restore = installElectron({ app });
    proxy = freshRequire('anonymity/proxy.js');
  });
  after(() => restore());

  it('turns a server proxy URL into host, port and type', () => {
    const out = proxy.normalizeProxy({ url: 'https://gate.test', username: 'u' });
    assert.deepEqual([out.type, out.host, out.port, out.username], ['https', 'gate.test', 443, 'u']);
    assert.equal(proxy.normalizeProxy({ url: 'socks5://s.test' }).port, 80);
    assert.equal(proxy.normalizeProxy({ url: 'http://g.test:7000' }).port, 7000);
  });

  it('passes a host/port config and no config through unchanged', () => {
    const config = { host: 'h', port: 1 };
    assert.equal(proxy.normalizeProxy(config), config);
    assert.equal(proxy.normalizeProxy(null), null);
  });

  it('goes direct when there is no proxy', async () => {
    await proxy.configureProxy(fakeSession(), null);
    assert.deepEqual(lastProxy, { mode: 'direct' });
  });

  it('routes SOCKS through socks5 and bypasses loopback', async () => {
    await proxy.configureProxy(fakeSession(), { type: 'socks', host: 's', port: 9 });
    assert.deepEqual(lastProxy, { proxyRules: 'socks5://s:9', proxyBypassRules: '<-loopback>' });
    await proxy.configureProxy(fakeSession(), { host: 'h', port: 8 });
    assert.equal(lastProxy.proxyRules, 'http://h:8');
  });

  it('answers only its own proxy challenge, and replaces the handler on reconfigure', async () => {
    const ses = fakeSession();
    await proxy.configureProxy(ses, { url: 'http://g.test:7000', username: 'u', password: 'p' });
    await proxy.configureProxy(ses, { url: 'http://g.test:7000', username: 'u2' });
    assert.equal(app.listenerCount('login'), 1);
    const answers = [];
    const challenge = (webContents, authInfo) =>
      app.emit('login', { preventDefault() {} }, webContents, {}, authInfo, (...a) => answers.push(a));
    challenge({ session: ses }, { isProxy: true, host: 'g.test', port: '7000' });
    challenge({ session: {} }, { isProxy: true, host: 'g.test', port: 7000 });
    challenge({ session: ses }, { isProxy: false, host: 'g.test', port: 7000 });
    assert.deepEqual(answers, [['u2', '']]);
    await proxy.configureProxy(ses, null);
    assert.equal(app.listenerCount('login'), 0);
  });

  it('meters a metered proxy through one loopback pass-through that counts both ways', async () => {
    const echo = net.createServer((s) => s.pipe(s));
    const servers = [];
    const createServer = net.createServer;
    mock.method(net, 'createServer', (...args) => servers.push(createServer(...args)) && servers.at(-1));
    await new Promise((r) => echo.listen(0, '127.0.0.1', r));
    const upstreamPort = echo.address().port;
    await proxy.configureProxy(fakeSession(), { host: '127.0.0.1', port: upstreamPort, metered: true });
    const port = await proxy.meter('127.0.0.1', upstreamPort);
    assert.equal(lastProxy.proxyRules, `http://127.0.0.1:${port}`);
    proxy.takeProxyBytes();
    const got = await new Promise((resolve) => {
      const c = net.connect(port, '127.0.0.1', () => c.write('abc'));
      c.once('data', (d) => c.end(() => resolve(d.toString())));
    });
    assert.equal(got, 'abc');
    await new Promise((r) => setTimeout(r, 20));
    assert.equal(proxy.takeProxyBytes(), 6);
    assert.equal(proxy.takeProxyBytes(), 0);
    assert.equal(await proxy.meter('127.0.0.1', upstreamPort), port, 'one meter per gateway');
    assert.equal(servers.length, 1);
    servers[0].close();
    echo.close();
    mock.restoreAll();
  });

  it('sets the DNS leak switches', () => {
    const switches = [];
    proxy.applyDNSLeakPrevention({ commandLine: { appendSwitch: (...a) => switches.push(a) } });
    assert.deepEqual(switches, [['disable-features', 'DnsOverHttps'], ['disable-async-dns']]);
  });

  it('adds to the features already switched off instead of replacing them', () => {
    const values = { 'disable-features': 'SafeBrowsing,Translate' };
    const commandLine = { getSwitchValue: (name) => values[name] || '', appendSwitch: (name, v) => (values[name] = v) };
    proxy.applyDNSLeakPrevention({ commandLine });
    assert.equal(values['disable-features'], 'SafeBrowsing,Translate,DnsOverHttps');
  });
});
