/**
 * Unit tests for `oya.control` (src/api/control.ts): each call hits its
 * endpoint with its method and body, ids encoded.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { client, type Call } from '../support/fake-fetch.ts';

/** Every request answers 200 with an empty object. */
const any = () => client(new Proxy({}, { get: () => ({ body: {} }) }));

/** The method, path and body of each call. */
const summary = (calls: Call[]) => calls.map((c) => [c.method, c.path, c.body]);

describe('oya.control', () => {
  it('reads sessions, overview and events', async () => {
    const { oya, calls } = any();
    await oya.control.overview();
    await oya.control.sessions();
    await oya.control.session('s:1');
    await oya.control.events();
    await oya.control.events(42);
    assert.deepEqual(summary(calls), [
      ['GET', '/api/control', undefined],
      ['GET', '/api/control/sessions', undefined],
      ['GET', '/api/control/sessions/s%3A1', undefined],
      ['GET', '/api/control/events?after=0', undefined],
      ['GET', '/api/control/events?after=42', undefined],
    ]);
  });

  it('steers a session: cancel, stop, takeover, input, recover, ticket', async () => {
    const { oya, calls } = any();
    await oya.control.cancel('s1');
    await oya.control.stop('s1');
    await oya.control.stop('s1', true);
    await oya.control.takeover('s1', 'acquire');
    await oya.control.input('s1', 'click', { x: 1 });
    await oya.control.recover('s1');
    await oya.control.ticket('s1');
    await oya.control.settings({ auditDays: 3 });
    assert.deepEqual(summary(calls), [
      ['POST', '/api/control/sessions/s1/cancel', {}],
      ['POST', '/api/control/sessions/s1/stop', { force: false }],
      ['POST', '/api/control/sessions/s1/stop', { force: true }],
      ['POST', '/api/control/sessions/s1/control', { action: 'acquire' }],
      ['POST', '/api/control/sessions/s1/input', { action: 'click', params: { x: 1 } }],
      ['POST', '/api/control/sessions/s1/recover', { replace: false }],
      ['POST', '/api/control/sessions/s1/ticket', {}],
      ['PATCH', '/api/control/project', { auditDays: 3 }],
    ]);
  });

  it('manages credentials, members and webhooks', async () => {
    const { oya, calls } = any();
    await oya.control.createCredential({ role: 'viewer', label: 'ci' });
    await oya.control.revokeCredential('c1');
    await oya.control.members();
    await oya.control.inviteMember();
    await oya.control.removeMember('u1');
    await oya.control.createWebhook('https://hook');
    await oya.control.removeWebhook('w1');
    await oya.control.replayDelivery('d1');
    assert.deepEqual(summary(calls), [
      ['POST', '/api/control/credentials', { role: 'viewer', label: 'ci' }],
      ['DELETE', '/api/control/credentials/c1', undefined],
      ['GET', '/api/control/members', undefined],
      ['POST', '/api/control/members/invite', { role: 'operator' }],
      ['DELETE', '/api/control/members/u1', undefined],
      ['POST', '/api/control/webhooks', { url: 'https://hook', types: [] }],
      ['DELETE', '/api/control/webhooks/w1', undefined],
      ['POST', '/api/control/deliveries/d1/replay', {}],
    ]);
  });
});

describe('oya.control.recover', () => {
  it('recovers in place, replaces with a boolean or options, and passes a cdp replacement its wsUrl', async () => {
    const { oya, calls } = client(new Proxy({}, { get: () => ({ body: {} }) }));
    await oya.control.recover('s1');
    await oya.control.recover('s1', true);
    await oya.control.recover('s1', { replace: true, wsUrl: 'ws://127.0.0.1:9222' });
    assert.deepEqual(
      calls.map((c) => c.body),
      [{ replace: false }, { replace: true }, { replace: true, wsUrl: 'ws://127.0.0.1:9222' }],
    );
  });

  it('refuses a wsUrl without replace before any request', async () => {
    const { oya, calls } = client();
    await assert.rejects(oya.control.recover('s1', { wsUrl: 'ws://x' }), {
      status: 400,
      message: /only used with replace/,
    });
    assert.equal(calls.length, 0);
  });
});
