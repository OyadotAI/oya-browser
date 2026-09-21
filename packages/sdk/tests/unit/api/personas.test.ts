/**
 * Unit tests for `oya.personas` (src/api/personas.ts) and its `profiles` alias.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { client, type Call } from '../support/fake-fetch.ts';

/** Every request answers 200 with `body`. */
const any = (body: unknown = {}) => client(new Proxy({}, { get: () => ({ body }) }));

/** The method, path and body of each call. */
const summary = (calls: Call[]) => calls.map((c) => [c.method, c.path, c.body]);

describe('oya.personas cookies', () => {
  it('exports a jar as stored or ready for Playwright, imports into one, and clears one', async () => {
    const { oya, calls } = any({ cookies: [{ name: 'sid' }], imported: 1 });
    assert.deepEqual(await oya.personas.cookies('p1'), [{ name: 'sid' }]);
    await oya.personas.cookies('p1', 'playwright');
    assert.equal((await oya.personas.importCookies('p2', [{ name: 'sid', value: 'v', domain: '.x.com' }])).imported, 1);
    await oya.personas.clearCookies('p2');
    assert.deepEqual(summary(calls), [
      ['GET', '/api/pool/cookies?persona=p1&format=json', undefined],
      ['GET', '/api/pool/cookies?persona=p1&format=playwright', undefined],
      ['PUT', '/api/pool/cookies?persona=p2', { cookies: [{ name: 'sid', value: 'v', domain: '.x.com' }] }],
      ['DELETE', '/api/pool/cookies?persona=p2', undefined],
    ]);
  });

  it("copies one persona's logins into another", async () => {
    const { oya, calls } = any({ cookies: [{ name: 'sid', value: 'v', domain: '.x.com' }], imported: 1 });
    assert.equal((await oya.personas.copyCookies('p1', 'p2')).imported, 1);
    assert.deepEqual(summary(calls), [
      ['GET', '/api/pool/cookies?persona=p1&format=json', undefined],
      ['PUT', '/api/pool/cookies?persona=p2', { cookies: [{ name: 'sid', value: 'v', domain: '.x.com' }] }],
    ]);
  });
});

describe('oya.personas', () => {
  it('unwraps the listing, the preview and a persona', async () => {
    const { oya } = any({ personas: [{ id: 'p1' }], fingerprint: { platform: 'Win32' } });
    assert.deepEqual(await oya.personas.list(), [{ id: 'p1' }]);
    assert.deepEqual(await oya.personas.preview({ platform: 'Win32' }), { platform: 'Win32' });
  });

  it('creates, updates, clones, pins and removes by id', async () => {
    const { oya, calls } = any();
    await oya.personas.get('p1');
    await oya.personas.create();
    await oya.personas.update('p1', { maxConcurrent: null });
    await oya.personas.clone('p1');
    await oya.personas.options();
    await oya.personas.pinProxy('p1', null);
    await oya.personas.remove('p1');
    assert.deepEqual(summary(calls), [
      ['GET', '/api/personas/p1', undefined],
      ['POST', '/api/personas', {}],
      ['PUT', '/api/personas/p1', { maxConcurrent: null }],
      ['POST', '/api/personas/p1/clone', {}],
      ['GET', '/api/personas/options', undefined],
      ['PUT', '/api/personas/p1/proxy', { proxyId: null }],
      ['DELETE', '/api/personas/p1', undefined],
    ]);
  });

  it('stores and clears factors and logins, scoping a clear to a domain', async () => {
    const { oya, calls } = any();
    await oya.personas.setMfa('p1', { type: 'totp', secret: 'S' });
    await oya.personas.clearMfa('p1');
    await oya.personas.clearMfa('p1', 'a b.com');
    await oya.personas.setCredentials('p1', { domain: 'x.com', username: 'u', password: 'pw' });
    await oya.personas.credentials('p1');
    await oya.personas.clearCredentials('p1', 'x.com');
    assert.deepEqual(summary(calls), [
      ['PUT', '/api/personas/p1/mfa', { type: 'totp', secret: 'S' }],
      ['DELETE', '/api/personas/p1/mfa', undefined],
      ['DELETE', '/api/personas/p1/mfa?domain=a%20b.com', undefined],
      ['PUT', '/api/personas/p1/credentials', { domain: 'x.com', username: 'u', password: 'pw' }],
      ['GET', '/api/personas/p1/credentials', undefined],
      ['DELETE', '/api/personas/p1/credentials?domain=x.com', undefined],
    ]);
  });

  it('profiles is the same object as personas', () => {
    const { oya } = any();
    assert.equal(oya.profiles, oya.personas);
  });
});
