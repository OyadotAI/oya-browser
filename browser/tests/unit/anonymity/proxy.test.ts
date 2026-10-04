/**
 * Unit tests for src/anonymity/proxy.ts: proxy shapes, session configuration and
 * auth through a fake Electron app, and the loopback byte meter.
 */
import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import net from 'node:net';
import { EventEmitter } from 'node:events';
import * as proxy from '../../../src/anonymity/proxy.ts';

/** A session that records every setProxy call. */
const fakeSession = (): any => ({ proxies: [], setProxy: async (value: unknown) => fakeSessionLast(value) });
let lastProxy: any;
/** Keeps the last proxy config set on any fake session. */
const fakeSessionLast = (value: unknown) => (lastProxy = value);

describe('proxy', () => {
  const app: any = new EventEmitter();
  /** Configures a fake session through the fake app, as main does through Electron's. */
  const configure = (ses: any, config: any) => proxy.configureProxy(ses, config, app);

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
    await configure(fakeSession(), null);
    assert.deepEqual(lastProxy, { mode: 'direct' });
  });

  it('routes SOCKS through socks5 and bypasses loopback', async () => {
    await configure(fakeSession(), { type: 'socks', host: 's', port: 9 });
    assert.deepEqual(lastProxy, { proxyRules: 'socks5://s:9', proxyBypassRules: '<-loopback>' });
    await configure(fakeSession(), { host: 'h', port: 8 });
    assert.equal(lastProxy.proxyRules, 'http://h:8');
  });

  it('answers only its own proxy challenge, and replaces the handler on reconfigure', async () => {
    const ses = fakeSession();
    await configure(ses, { url: 'http://g.test:7000', username: 'u', password: 'p' });
    await configure(ses, { url: 'http://g.test:7000', username: 'u2' });
    assert.equal(app.listenerCount('login'), 1);
    const answers: unknown[] = [];
    const challenge = (webContents: any, authInfo: any) =>
      app.emit('login', { preventDefault() {} }, webContents, {}, authInfo, (...a: unknown[]) => answers.push(a));
    challenge({ session: ses }, { isProxy: true, host: 'g.test', port: '7000' });
    challenge({ session: {} }, { isProxy: true, host: 'g.test', port: 7000 });
    challenge({ session: ses }, { isProxy: false, host: 'g.test', port: 7000 });
    assert.deepEqual(answers, [['u2', '']]);
    await configure(ses, null);
    assert.equal(app.listenerCount('login'), 0);
  });

  it('meters a metered proxy through one loopback pass-through that counts both ways', async () => {
    const echo = net.createServer((s) => s.pipe(s));
    const servers: net.Server[] = [];
    const createServer = net.createServer;
    mock.method(net, 'createServer', (...args: any[]) => servers.push(createServer(...args)) && servers.at(-1));
    await new Promise<void>((r) => echo.listen(0, '127.0.0.1', r));
    const upstreamPort = (echo.address() as net.AddressInfo).port;
    await configure(fakeSession(), { host: '127.0.0.1', port: upstreamPort, metered: true });
    const port = await proxy.meter('127.0.0.1', upstreamPort);
    assert.equal(lastProxy.proxyRules, `http://127.0.0.1:${port}`);
    proxy.takeProxyBytes();
    const got = await new Promise<string>((resolve) => {
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
    const switches: unknown[] = [];
    proxy.applyDNSLeakPrevention({ commandLine: { appendSwitch: (...a: unknown[]) => switches.push(a) } });
    assert.deepEqual(switches, [['disable-features', 'DnsOverHttps'], ['disable-async-dns']]);
  });

  it('adds to the features already switched off instead of replacing them', () => {
    const values: Record<string, string> = { 'disable-features': 'SafeBrowsing,Translate' };
    const commandLine = {
      getSwitchValue: (name: string) => values[name] || '',
      appendSwitch: (name: string, v: string) => (values[name] = v),
    };
    proxy.applyDNSLeakPrevention({ commandLine } as any);
    assert.equal(values['disable-features'], 'SafeBrowsing,Translate,DnsOverHttps');
  });
});
