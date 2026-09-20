/**
 * Unit tests for the audit log: actors are fingerprinted, long fields are cut,
 * the client address comes from the request, recent() filters newest first,
 * and without a database the trail goes to audit.log.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { audit, drain, fingerprint, history, recent } from '../../../src/platform/audit.ts';
import { dataPath } from '../../../src/platform/paths.ts';
import { AUDIT_FIELD_MAX_CHARS, FINGERPRINT_HEX_CHARS } from '../../../src/platform/constants.ts';

describe('fingerprint', () => {
  it('is a short, stable hex digest of the key', () => {
    const f = fingerprint('key-a');
    assert.match(f, new RegExp(`^[0-9a-f]{${FINGERPRINT_HEX_CHARS}}$`));
    assert.equal(fingerprint('key-a'), f);
    assert.notEqual(fingerprint('key-b'), f);
  });

  it('is null for no key', () => {
    assert.equal(fingerprint(''), null);
  });
});

describe('audit', () => {
  it('records the actor as a fingerprint, never the key', () => {
    const row = audit({ action: 'key.create', actorKey: 'secret-key' });
    assert.equal(row.actor, fingerprint('secret-key'));
    assert.ok(!JSON.stringify(row).includes('secret-key'));
  });

  it('defaults the outcome to ok and the optional fields to null', () => {
    const row = audit({ action: 'config.update' });
    assert.equal(row.outcome, 'ok');
    assert.deepEqual([row.actor, row.target_type, row.target_id, row.ip, row.meta], [null, null, null, null, null]);
  });

  it('cuts the target id and user agent to the field limit', () => {
    const row = audit({
      action: 'browser.stop',
      targetId: 'x'.repeat(500),
      req: { headers: { 'user-agent': 'u'.repeat(500) } },
    });
    assert.equal(row.target_id.length, AUDIT_FIELD_MAX_CHARS);
    assert.equal(row.user_agent.length, AUDIT_FIELD_MAX_CHARS);
  });

  it('takes the client address from the first x-forwarded-for hop, else the socket', () => {
    const forwarded = audit({ action: 'a', req: { headers: { 'x-forwarded-for': ' 1.2.3.4 , 10.0.0.1' } } });
    const direct = audit({ action: 'a', req: { headers: {}, socket: { remoteAddress: '5.6.7.8' } } });
    assert.equal(forwarded.ip, '1.2.3.4');
    assert.equal(direct.ip, '5.6.7.8');
  });

  it('keeps a copy of meta, not the caller’s object', () => {
    const meta = { n: 1 };
    const row = audit({ action: 'a', meta });
    meta.n = 2;
    assert.deepEqual(row.meta, { n: 1 });
  });
});

describe('recent', () => {
  it('returns newest first, filtered by action, actor and outcome', () => {
    audit({ action: 'recent.test', actorKey: 'k1', outcome: 'ok', targetId: 'first' });
    audit({ action: 'recent.test', actorKey: 'k2', outcome: 'denied', targetId: 'second' });
    audit({ action: 'recent.test', actorKey: 'k1', outcome: 'error', targetId: 'third' });
    const all = recent({ action: 'recent.test' });
    assert.deepEqual(
      all.map((e) => e.target_id),
      ['third', 'second', 'first'],
    );
    assert.equal(recent({ action: 'recent.test', actor: fingerprint('k1') }).length, 2);
    assert.equal(recent({ action: 'recent.test', outcome: 'denied' })[0].target_id, 'second');
  });

  it('honours the limit', () => {
    for (let i = 0; i < 3; i++) audit({ action: 'limit.test' });
    assert.equal(recent({ action: 'limit.test', limit: 2 }).length, 2);
  });
});

describe('history and drain', () => {
  it('answers history from memory when there is no database', async () => {
    audit({ action: 'history.test' });
    const out = await history({ action: 'history.test' });
    assert.equal(out.source, 'memory');
    assert.equal(out.events.length, 1);
  });

  it('appends pending events to audit.log on drain', async () => {
    audit({ action: 'drain.test', targetId: 'd-1' });
    await drain();
    const lines = readFileSync(dataPath('audit.log'), 'utf8')
      .trim()
      .split('\n')
      .map((l) => JSON.parse(l));
    assert.ok(lines.some((e) => e.action === 'drain.test' && e.target_id === 'd-1'));
  });
});
