/**
 * Unit tests for the caller's address: the hop our own proxies appended, never
 * one a client wrote, with the count of trusted proxies set by
 * OYA_TRUSTED_PROXY_HOPS.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { clientAddress, forwardedClient } from '../../../src/platform/client-address.ts';

/** A request carrying `forwarded` as its X-Forwarded-For, from a socket at 10.0.0.1. */
const req = (forwarded?: string) => ({
  headers: forwarded === undefined ? {} : { 'x-forwarded-for': forwarded },
  socket: { remoteAddress: '10.0.0.1' },
});

afterEach(() => {
  delete process.env.OYA_TRUSTED_PROXY_HOPS;
});

describe('clientAddress', () => {
  it('defaults to one trusted hop: the last one, which the ingress appended', () => {
    assert.equal(clientAddress(req('6.6.6.6, 203.0.113.9')), '203.0.113.9');
  });

  it('takes the hop the outermost trusted proxy appended', () => {
    process.env.OYA_TRUSTED_PROXY_HOPS = '2';
    assert.equal(clientAddress(req('203.0.113.9, 10.128.0.4')), '203.0.113.9');
  });

  it('ignores hops the client prepended', () => {
    process.env.OYA_TRUSTED_PROXY_HOPS = '2';
    assert.equal(clientAddress(req('6.6.6.6, 7.7.7.7, 203.0.113.9, 10.128.0.4')), '203.0.113.9');
  });

  it('falls back to the nearest proxy when the chain is shorter than the trusted hops', () => {
    process.env.OYA_TRUSTED_PROXY_HOPS = '2';
    assert.equal(clientAddress(req('10.128.0.4')), '10.128.0.4');
  });

  it('falls back to the socket without a forwarded header, and to unknown without either', () => {
    assert.equal(clientAddress(req()), '10.0.0.1');
    assert.equal(clientAddress({ headers: {} }), 'unknown');
    assert.equal(forwardedClient({ headers: {} }), null);
  });

  it('falls back to the socket when the chosen hop is not an address', () => {
    process.env.OYA_TRUSTED_PROXY_HOPS = '2';
    assert.equal(clientAddress(req('<script>, 10.128.0.4')), '10.0.0.1');
  });

  it('treats a non-positive or non-numeric hop count as the default', () => {
    for (const bad of ['0', '-2', 'many']) {
      process.env.OYA_TRUSTED_PROXY_HOPS = bad;
      assert.equal(clientAddress(req('6.6.6.6, 203.0.113.9')), '203.0.113.9', bad);
    }
  });
});
