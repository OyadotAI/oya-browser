/**
 * Unit tests for the proxy pool facade (service.ts with registration.ts):
 * registering and sealing proxies, per-owner visibility, sticky assignment to
 * personas, and health checks through a loopback proxy.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir, restoreEnv } from '../../support/data-dir.ts';
import { connectProxy, targetServer } from '../../support/proxies.ts';

ownDataDir('oya-proxies-');
const proxies = await import('../../../../src/modules/proxies/service.ts');
const store = await import('../../../../src/modules/proxies/store.ts');
const { Proxy } = await import('../../../../src/modules/proxies/proxy.ts');

const PUBLIC = 'http://user:secret@8.8.8.8:3128';

/** A proxy put straight into the pool. */
function add(id: string, cfg: Record<string, unknown> = {}) {
  const p = new Proxy({ id, sealed: 'x', ...cfg });
  store.proxies.set(id, p);
  return p;
}

beforeEach(() => proxies.reset());
afterEach(() => {
  mock.timers.reset();
  mock.restoreAll();
});

describe('register', () => {
  it('registers a proxy with its credentials split out of the URL and sealed', async () => {
    const p = await proxies.register({
      owner: 'o',
      label: 'Mine',
      url: PUBLIC,
      geo: 'US',
      kind: 'datacenter',
      maxPersonas: 2,
    });
    assert.deepEqual([p.owner, p.label, p.geo, p.kind, p.maxPersonas], ['o', 'Mine', 'US', 'datacenter', 2]);
    assert.doesNotMatch(p.sealed, /secret|user/);
    assert.deepEqual(proxies.credentials(p), { url: 'http://8.8.8.8:3128', username: 'user', password: 'secret' });
    assert.equal(store.proxies.get(p.id), p);
  });

  it('decodes percent-encoded credentials, so the proxy gets the password as written', async () => {
    const p = await proxies.register({ url: 'http://us%40er:pa%3Ass%40w@8.8.8.8:3128' } as any);
    assert.deepEqual(proxies.credentials(p), { url: 'http://8.8.8.8:3128', username: 'us@er', password: 'pa:ss@w' });
  });

  it('keeps null credentials for a proxy without them', async () => {
    const p = await proxies.register({ url: 'http://8.8.8.8:3128' } as any);
    assert.deepEqual(proxies.credentials(p), { url: 'http://8.8.8.8:3128', username: null, password: null });
    assert.equal(p.owner, null);
  });

  it('refuses a URL that does not parse, with a 400', async () => {
    await assert.rejects(proxies.register({ url: 'not a url' } as any), {
      status: 400,
      message: 'proxy url must be a valid URL',
    });
  });

  it('refuses a scheme Chromium cannot use, with a 400', async () => {
    await assert.rejects(proxies.register({ url: 'ftp://8.8.8.8:21' } as any), {
      status: 400,
      message: 'proxy url must be http, https or socks5',
    });
  });

  it('refuses SOCKS with a username or password, which Chromium would drop', async () => {
    await assert.rejects(proxies.register({ url: 'socks5://u:p@8.8.8.8:1080' } as any), {
      status: 400,
      message: /Chromium cannot authenticate SOCKS5 proxies/,
    });
    await assert.doesNotReject(proxies.register({ url: 'socks5://8.8.8.8:1080' } as any));
  });

  it('refuses a loopback proxy unless the host opts in', async () => {
    const saved = process.env.OYA_ALLOW_PRIVATE_TARGETS;
    delete process.env.OYA_ALLOW_PRIVATE_TARGETS;
    await assert.rejects(proxies.register({ url: 'http://127.0.0.1:3128' } as any), {
      status: 400,
      message: /^proxy url/,
    });
    process.env.OYA_ALLOW_PRIVATE_TARGETS = 'true';
    await assert.doesNotReject(proxies.register({ url: 'http://127.0.0.1:3128' } as any));
    restoreEnv('OYA_ALLOW_PRIVATE_TARGETS', saved);
  });

  it('never allows the cloud metadata address', async () => {
    process.env.OYA_ALLOW_PRIVATE_TARGETS = 'true';
    await assert.rejects(proxies.register({ url: 'http://169.254.169.254:80' } as any), { status: 400 });
    delete process.env.OYA_ALLOW_PRIVATE_TARGETS;
  });
});

describe('visibility and removal', () => {
  it("shows an owner their own proxies and shared ones, never another owner's", () => {
    add('px-mine', { owner: 'a' });
    add('px-shared', { owner: null });
    add('px-theirs', { owner: 'b' });
    assert.deepEqual(
      proxies.visible('a').map((p) => p.id),
      ['px-mine', 'px-shared'],
    );
    assert.deepEqual(
      proxies.list('a').map((p) => p.id),
      ['px-mine', 'px-shared'],
    );
    assert.equal('sealed' in proxies.list('a')[0], false);
  });

  it('removes an owner’s proxy and drops its persona assignments', () => {
    add('px-mine', { owner: 'a' });
    proxies.assign('a', 'p-1', 'px-mine');
    assert.equal(proxies.remove('a', 'px-mine'), true);
    assert.equal(store.proxies.has('px-mine'), false);
    assert.equal(proxies.assigned('p-1'), null);
  });

  it("will not remove another owner's proxy, or one that does not exist", () => {
    add('px-theirs', { owner: 'b' });
    assert.equal(proxies.remove('a', 'px-theirs'), false);
    assert.equal(proxies.remove('a', 'px-missing'), false);
    assert.equal(store.proxies.has('px-theirs'), true);
  });

  it('refuses to remove a shared proxy with a 403', () => {
    add('px-shared', { owner: null });
    assert.throws(() => proxies.remove('a', 'px-shared'), {
      status: 403,
      message: 'Shared proxies are host-configured',
    });
  });
});

describe('forPersona', () => {
  it('is null when no proxy is configured', () => {
    assert.equal(proxies.forPersona('a', { id: 'p-1' }), null);
  });

  it('assigns the least-used available proxy and keeps it for the persona', () => {
    const busy = add('px-busy', { owner: 'a', maxPersonas: 5 });
    add('px-free', { owner: 'a', maxPersonas: 5 });
    store.assignments.set('p-other', busy.id);
    const first = proxies.forPersona('a', { id: 'p-1' });
    assert.equal(first.id, 'px-free');
    store.assignments.set('p-x', 'px-free');
    store.assignments.set('p-y', 'px-free');
    assert.equal(proxies.forPersona('a', { id: 'p-1' }), first, 'sticky even when it is no longer least used');
  });

  it('skips proxies that are full or unavailable', () => {
    add('px-full', { owner: 'a', maxPersonas: 1 });
    store.assignments.set('p-other', 'px-full');
    add('px-down', { owner: 'a', healthy: false });
    assert.equal(proxies.forPersona('a', { id: 'p-1' }), null);
  });

  it('prefers the geo asked for, or the persona’s own proxy geo, matching a region by prefix', () => {
    add('px-de', { owner: 'a', geo: 'DE' });
    add('px-us', { owner: 'a', geo: 'US-CA' });
    assert.equal(proxies.forPersona('a', { id: 'p-1' }, { geo: 'US' }).id, 'px-us');
    assert.equal(proxies.forPersona('a', { id: 'p-2', proxy: { geo: 'DE' } }).id, 'px-de');
    assert.equal(proxies.forPersona('a', { id: 'p-3' }, { geo: 'JP' }), null);
  });

  it("never assigns another owner's proxy", () => {
    add('px-theirs', { owner: 'b' });
    assert.equal(proxies.forPersona('a', { id: 'p-1' }), null);
  });

  it('reassigns, with a warning, when the kept proxy has gone down', () => {
    const warn = mock.method(console, 'warn', () => {});
    const kept = add('px-kept', { owner: 'a' });
    add('px-spare', { owner: 'a' });
    store.assignments.set('p-1', kept.id);
    kept.healthy = false;
    assert.equal(proxies.forPersona('a', { id: 'p-1' }).id, 'px-spare');
    assert.match(
      warn.mock.calls[0].arguments[0],
      /p-1 was on px-kept, which is unhealthy — reassigning changes its exit IP/,
    );
  });
});

describe('assign, unassign and assigned', () => {
  it("pins a persona to the owner's or a shared proxy", () => {
    add('px-mine', { owner: 'a' });
    add('px-shared', { owner: null });
    assert.equal(proxies.assign('a', 'p-1', 'px-mine').id, 'px-mine');
    assert.equal(proxies.assigned('p-1').id, 'px-mine');
    assert.equal(proxies.assign('a', 'p-1', 'px-shared').id, 'px-shared');
  });

  it("refuses another owner's proxy or a missing one", () => {
    add('px-theirs', { owner: 'b' });
    assert.equal(proxies.assign('a', 'p-1', 'px-theirs'), null);
    assert.equal(proxies.assign('a', 'p-1', 'px-missing'), null);
    assert.equal(proxies.assigned('p-1'), null);
  });

  it('forgets a pin on unassign, and reports nothing for a proxy since removed', () => {
    add('px-mine', { owner: 'a' });
    proxies.assign('a', 'p-1', 'px-mine');
    proxies.unassign('p-1');
    assert.equal(proxies.assigned('p-1'), null);
    store.assignments.set('p-2', 'px-gone');
    assert.equal(proxies.assigned('p-2'), null);
  });
});

describe('check', () => {
  /** A registered loopback proxy in front of a loopback target answering `body`. */
  async function loopback(mode: 'tunnel' | 'refuse' = 'tunnel', body?: string) {
    const proxy = await connectProxy(mode);
    const target = await targetServer(body);
    process.env.OYA_ALLOW_PRIVATE_TARGETS = 'true';
    process.env.OYA_PROXY_CHECK_URL = `http://127.0.0.1:${target.port}/`;
    const p = await proxies.register({ owner: 'a', url: `http://u:p@127.0.0.1:${proxy.port}` } as any);
    const close = async () => {
      delete process.env.OYA_ALLOW_PRIVATE_TARGETS;
      delete process.env.OYA_PROXY_CHECK_URL;
      await Promise.all([proxy.close(), target.close()]);
    };
    return { p, proxy, close };
  }

  it('learns the exit IP and marks the proxy healthy', async () => {
    const { p, proxy, close } = await loopback();
    p.fail();
    assert.deepEqual(await proxies.check(p), { ok: true, exitIp: '203.0.113.9' });
    assert.deepEqual([p.healthy, p.exitIp], [true, '203.0.113.9']);
    assert.equal(proxy.auth[0], `Basic ${Buffer.from('u:p').toString('base64')}`);
    await close();
  });

  it("reads an httpbin-style 'origin' as the exit IP", async () => {
    const { p, close } = await loopback('tunnel', '{"origin":"198.51.100.1"}');
    assert.equal((await proxies.check(p)).exitIp, '198.51.100.1');
    await close();
  });

  it('puts a proxy that fails its check into cooldown', async () => {
    const { p, close } = await loopback('refuse');
    assert.deepEqual(await proxies.check(p), { ok: false, error: 'proxy answered 407' });
    assert.deepEqual([p.healthy, p.failures], [false, 1]);
    await close();
  });

  it('checks every proxy the owner can see', async () => {
    const { p, close } = await loopback();
    assert.deepEqual(await proxies.checkAll('a'), [{ id: p.id, ok: true, exitIp: '203.0.113.9' }]);
    assert.deepEqual(await proxies.checkAll('b'), []);
    await close();
  });
});
