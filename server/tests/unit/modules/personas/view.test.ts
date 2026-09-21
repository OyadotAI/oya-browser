/**
 * Unit tests for the public view of a persona: the fingerprint is described
 * without its seed, a proxy without its credentials, and the exit it
 * actually uses is shown.
 */
import { describe, it, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { describePersona } from '../../../../src/modules/personas/view.ts';
import { describeProfile } from '../../../../src/modules/personas/model.ts';
import { personaService } from '../../support/personas.ts';

beforeEach(() => mock.method(console, 'log', () => {}));

describe('describePersona', () => {
  it('describes identity, usage, prefs and fingerprint, never the seed', () => {
    const { service, deps } = personaService();
    const p = service.create('k', { name: 'Work', prefs: { platform: 'MacIntel' }, maxConcurrent: 3 });
    service.acquire(p, 'b-1');
    const view = describePersona(p, service, deps);
    assert.equal('seed' in view, false);
    assert.equal('owner' in view, false);
    assert.deepEqual(
      [view.id, view.name, view.isDefault, view.activeBrowsers, view.maxConcurrent],
      [p.id, 'Work', false, 1, 3],
    );
    assert.deepEqual(view.prefs, { platform: 'MacIntel' });
    assert.deepEqual(view.fingerprint, describeProfile(service.fingerprintFor(p)));
  });

  it("shows the persona's own proxy without its credentials", () => {
    const { service, deps } = personaService();
    const p = service.create('k', { proxy: { host: 'h', port: 8080, username: 'u', password: 'secret' } });
    const view = describePersona(p, service, deps);
    assert.deepEqual(view.proxy, { host: 'h', port: 8080, geo: null });
    assert.doesNotMatch(JSON.stringify(view), /secret/);
  });

  it('shows the proxy the persona is assigned to as its exit', () => {
    const assigned = { id: 'px-1', label: 'Denver', geo: 'US', available: true, secret: 'x' };
    const { service, deps } = personaService({
      proxies: { assigned: () => assigned, residential: () => ({ geo: 'DE' }) },
    });
    const view = describePersona(service.create('k'), service, deps);
    assert.deepEqual(view.exit, { id: 'px-1', label: 'Denver', geo: 'US', healthy: true });
  });

  it('shows the residential exit a persona without a proxy falls back to', () => {
    const { service, deps } = personaService({ proxies: { assigned: () => null, residential: () => ({ geo: 'DE' }) } });
    const view = describePersona(service.create('k'), service, deps);
    assert.deepEqual(view.exit, { id: 'residential', label: 'Oya residential', geo: 'DE', healthy: true });
  });

  it('shows no residential exit for a persona with its own proxy host', () => {
    const { service, deps } = personaService({ proxies: { assigned: () => null, residential: () => ({ geo: 'DE' }) } });
    const view = describePersona(service.create('k', { proxy: { host: 'h' } }), service, deps);
    assert.equal(view.exit, null);
  });

  it('shows no exit when there is none to fall back to', () => {
    const { service, deps } = personaService();
    assert.equal(describePersona(service.create('k'), service, deps).exit, null);
  });

  it('shows what the persona can sign in with, from the stores it is given', () => {
    const { service, deps } = personaService({
      mfa: { describe: () => ({ configured: true, type: 'totp' }), list: () => [{ domain: 'a.com' }], clearAll() {} },
      credentials: { list: () => [{ domain: 'a.com', username: 'me' }], clearAll() {} },
      logins: { summary: () => ({ cookies: 3 }), clear() {} },
    });
    const view = describePersona(service.create('k'), service, deps);
    assert.deepEqual(view.mfa, { configured: true, type: 'totp' });
    assert.deepEqual(view.sites, { mfa: [{ domain: 'a.com' }], credentials: [{ domain: 'a.com', username: 'me' }] });
    assert.deepEqual(view.login, { cookies: 3 });
  });

  it('is what PersonaService.describe returns', () => {
    const { service, deps } = personaService();
    const p = service.create('k');
    assert.deepEqual(service.describe(p), describePersona(p, service, deps));
  });
});
