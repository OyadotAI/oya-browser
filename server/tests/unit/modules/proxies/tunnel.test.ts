/**
 * Unit tests for getVia: a GET tunnelled through an http proxy with CONNECT,
 * against a loopback proxy and target, including refusals and the timeout.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { connectProxy, targetServer } from '../../support/proxies.ts';

process.env.OYA_PROXY_CHECK_TIMEOUT_MS = '200';
const { getVia } = await import('../../../../src/modules/proxies/tunnel.ts');

const cleanup: (() => Promise<void>)[] = [];
/** Closes what a test opened. */
afterEach(async () => {
  await Promise.all(cleanup.splice(0).map((close) => close()));
});

/** A proxy and a target, registered for cleanup. */
async function pair(mode: 'tunnel' | 'refuse' | 'silent' = 'tunnel', body?: string, status?: number) {
  const proxy = await connectProxy(mode);
  const target = await targetServer(body, status);
  cleanup.push(proxy.close, target.close);
  return {
    proxy,
    target,
    proxyUrl: new URL(`http://127.0.0.1:${proxy.port}`),
    url: new URL(`http://127.0.0.1:${target.port}/ip?format=json`),
  };
}

describe('getVia', () => {
  it('fetches the target through the proxy and resolves with its body', async () => {
    const { proxyUrl, url, target } = await pair();
    assert.equal(await getVia(proxyUrl, null, null, url), '{"ip":"203.0.113.9"}');
    assert.deepEqual(target.paths, ['/ip?format=json']);
  });

  it('sends basic proxy credentials when it has a username', async () => {
    const { proxyUrl, url, proxy } = await pair();
    await getVia(proxyUrl, 'user', 'pass', url);
    await getVia(proxyUrl, 'solo', null, url);
    await getVia(proxyUrl, null, null, url);
    assert.deepEqual(proxy.auth, [
      `Basic ${Buffer.from('user:pass').toString('base64')}`,
      `Basic ${Buffer.from('solo:').toString('base64')}`,
      undefined,
    ]);
  });

  it('fails when the proxy refuses the CONNECT', async () => {
    const { proxyUrl, url } = await pair('refuse');
    await assert.rejects(getVia(proxyUrl, 'u', 'wrong', url), /proxy answered 407/);
  });

  it('fails when the target answers anything but 200', async () => {
    const { proxyUrl, url } = await pair('tunnel', 'nope', 500);
    await assert.rejects(getVia(proxyUrl, null, null, url), /check returned 500/);
  });

  it('gives up when the proxy never answers', async () => {
    const { proxyUrl, url } = await pair('silent');
    await assert.rejects(getVia(proxyUrl, null, null, url), /proxy check timed out/);
  });

  it('fails when nothing listens where the proxy should be', async () => {
    const { proxy, url } = await pair();
    await proxy.close();
    await assert.rejects(getVia(new URL(`http://127.0.0.1:${proxy.port}`), null, null, url), /ECONNREFUSED/);
  });
});
