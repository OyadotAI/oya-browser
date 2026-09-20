/**
 * Unit tests for the outbound destination guard: which addresses are never
 * dialled, which are private, and how assertSafeTarget checks a caller's URL
 * (scheme, credentials, DNS answers, the private-targets opt-in).
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import dns from 'node:dns';
import { syncBuiltinESMExports } from 'node:module';
import { assertSafeTarget, isNeverAllowed, isPrivateAddress } from '../../../src/platform/net-guard.ts';

/** Makes DNS answer every lookup with `addresses` (or fail when null) until the test ends. */
function resolveTo(addresses: string[] | null) {
  mock.method(dns.promises, 'lookup', async () => {
    if (!addresses) throw new Error('ENOTFOUND');
    return addresses.map((address) => ({ address, family: address.includes(':') ? 6 : 4 }));
  });
  syncBuiltinESMExports();
}

describe('isNeverAllowed', () => {
  it('refuses the cloud metadata range and "this network"', () => {
    assert.equal(isNeverAllowed('169.254.169.254'), true);
    assert.equal(isNeverAllowed('0.0.0.0'), true);
  });

  it('refuses multicast and reserved IPv4', () => {
    assert.equal(isNeverAllowed('224.0.0.1'), true);
    assert.equal(isNeverAllowed('255.255.255.255'), true);
  });

  it('refuses IPv6 link-local, the unspecified address and a mapped metadata address', () => {
    assert.equal(isNeverAllowed('fe80::1'), true);
    assert.equal(isNeverAllowed('::'), true);
    assert.equal(isNeverAllowed('::ffff:169.254.169.254'), true);
  });

  it('refuses anything that is not an IP', () => {
    assert.equal(isNeverAllowed('example.com'), true);
  });

  it('leaves loopback and private ranges to the opt-in', () => {
    assert.equal(isNeverAllowed('127.0.0.1'), false);
    assert.equal(isNeverAllowed('10.1.2.3'), false);
    assert.equal(isNeverAllowed('::1'), false);
  });
});

describe('isPrivateAddress', () => {
  it('flags loopback, RFC1918, CGNAT and benchmarking ranges', () => {
    for (const ip of [
      '127.0.0.1',
      '10.0.0.5',
      '172.16.0.1',
      '172.31.255.255',
      '192.168.1.1',
      '100.64.0.1',
      '198.18.0.1',
    ])
      assert.equal(isPrivateAddress(ip), true, ip);
  });

  it('passes public addresses, including the edges of private ranges', () => {
    for (const ip of ['8.8.8.8', '172.15.0.1', '172.32.0.1', '100.128.0.1', '1.1.1.1'])
      assert.equal(isPrivateAddress(ip), false, ip);
  });

  it('flags IPv6 loopback, unique local and link-local, and judges a mapped address as IPv4', () => {
    for (const ip of ['::1', 'fc00::1', 'fd12::1', 'fe80::1', '::ffff:10.0.0.1'])
      assert.equal(isPrivateAddress(ip), true, ip);
    assert.equal(isPrivateAddress('2001:4860:4860::8888'), false);
    assert.equal(isPrivateAddress('::ffff:8.8.8.8'), false);
  });

  it('treats anything unresolvable as unsafe', () => {
    assert.equal(isPrivateAddress('not-an-ip'), true);
  });
});

describe('assertSafeTarget', () => {
  beforeEach(() => delete process.env.OYA_ALLOW_PRIVATE_TARGETS);
  afterEach(() => {
    mock.restoreAll();
    syncBuiltinESMExports();
    delete process.env.OYA_ALLOW_PRIVATE_TARGETS;
  });

  it('accepts a public IP literal without a DNS lookup', async () => {
    const out = await assertSafeTarget('wss://8.8.8.8/devtools');
    assert.deepEqual(out, { href: 'wss://8.8.8.8/devtools', hostname: '8.8.8.8', addresses: ['8.8.8.8'] });
  });

  it('resolves a hostname and returns every address', async () => {
    resolveTo(['93.184.216.34', '2606:2800:220:1::1']);
    const out = await assertSafeTarget('ws://example.com:9222/');
    assert.deepEqual(out.addresses, ['93.184.216.34', '2606:2800:220:1::1']);
  });

  it('refuses a value that is not a URL', async () => {
    await assert.rejects(assertSafeTarget('not a url'), { status: 400, message: 'URL is not a valid URL' });
  });

  it('refuses a scheme outside the allowed protocols, naming them', async () => {
    await assert.rejects(assertSafeTarget('http://8.8.8.8/'), { status: 400, message: 'URL must use ws: or wss:' });
  });

  it('refuses embedded credentials under the caller’s label', async () => {
    await assert.rejects(assertSafeTarget('wss://u:p@8.8.8.8/', { label: 'wsUrl' }), {
      message: 'wsUrl must not embed credentials',
    });
  });

  it('refuses a hostname that does not resolve', async () => {
    resolveTo(null);
    await assert.rejects(assertSafeTarget('ws://nowhere.test/'), /hostname could not be resolved \(nowhere.test\)/);
  });

  it('refuses a hostname that resolves to nothing', async () => {
    resolveTo([]);
    await assert.rejects(assertSafeTarget('ws://empty.test/'), /could not be resolved/);
  });

  it('refuses a public name that points at a private address', async () => {
    resolveTo(['10.0.0.5']);
    await assert.rejects(assertSafeTarget('ws://sneaky.test/'), /private or loopback address \(10.0.0.5\)/);
  });

  it('allows a private address once the host opts in', async () => {
    process.env.OYA_ALLOW_PRIVATE_TARGETS = 'true';
    const out = await assertSafeTarget('ws://127.0.0.1:9222/');
    assert.deepEqual(out.addresses, ['127.0.0.1']);
  });

  it('never allows the metadata service, even with the opt-in', async () => {
    process.env.OYA_ALLOW_PRIVATE_TARGETS = 'true';
    await assert.rejects(assertSafeTarget('ws://169.254.169.254/'), /link-local or reserved address/);
  });

  it('judges a bracketed IPv6 literal by its address', async () => {
    await assert.rejects(assertSafeTarget('ws://[::1]:9222/'), /private or loopback address \(::1\)/);
  });
});
