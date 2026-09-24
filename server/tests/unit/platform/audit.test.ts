/**
 * Unit tests for the audit log: actors are fingerprinted, long fields are cut,
 * the client address comes from the request, recent() filters newest first,
 * and the trail is written to storage, keeping a failed batch for next time.
 */
import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { audit, drain, fingerprint, history, recent } from '../../../src/platform/audit.ts';
import { getConnection } from '../../../src/platform/storage/index.ts';
import { AUDIT_FIELD_MAX_CHARS, FINGERPRINT_HEX_CHARS, MS_PER_SECOND } from '../../../src/platform/constants.ts';

/** Stored audit rows for one action. */
const storedRows = (action: string) => getConnection().select('audit_log', { action });

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
  it('writes pending events to storage on drain, hash-linked', async () => {
    audit({ action: 'drain.test', targetId: 'd-1' });
    await drain();
    const [row] = await storedRows('drain.test');
    assert.equal(row.target_id, 'd-1');
    assert.ok(row.hash && row.seq);
  });

  it('answers history from storage, filtered by action and actor, newest first', async () => {
    // A second apart, so newest-first has one answer.
    mock.timers.enable({ apis: ['Date'], now: Date.UTC(2026, 0, 1) });
    audit({ action: 'history.test', actorKey: 'k1', targetId: 'first' });
    mock.timers.tick(MS_PER_SECOND);
    audit({ action: 'history.test', actorKey: 'k2', targetId: 'other' });
    mock.timers.tick(MS_PER_SECOND);
    audit({ action: 'history.test', actorKey: 'k1', targetId: 'second' });
    mock.timers.reset();
    await drain();
    const out = await history({ action: 'history.test', actor: fingerprint('k1') });
    assert.equal(out.source, 'database');
    assert.deepEqual(
      out.events.map((e) => e.target_id),
      ['second', 'first'],
    );
  });

  it('answers history from memory, with the error, when storage cannot be read', async () => {
    audit({ action: 'history.memory' });
    const select = mock.method(getConnection(), 'select', async () => Promise.reject(new Error('storage down')));
    const out = await history({ action: 'history.memory' });
    select.mock.restore();
    assert.deepEqual([out.source, out.error, out.events.length], ['memory', 'storage down', 1]);
  });

  it('keeps a batch storage refused for the next flush, without scheduling one itself', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    const error = mock.method(console, 'error', () => {});
    const upsert = mock.method(getConnection(), 'upsert', async () => Promise.reject(new Error('storage down')));
    audit({ action: 'requeue.test' });
    await drain();
    mock.timers.runAll();
    assert.equal(upsert.mock.callCount(), 1, 'a failed write does not retry on its own');
    assert.match(error.mock.calls[0].arguments[0], /write failed \(storage down\)/);
    upsert.mock.restore();
    await drain();
    assert.equal((await storedRows('requeue.test')).length, 1);
    error.mock.restore();
    mock.timers.reset();
  });
});
