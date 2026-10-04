/**
 * Unit tests for the audit log: actors are fingerprinted, long fields are cut,
 * the client address comes from the request, the authenticating credential is
 * kept, recent() filters newest first, the trail is written to storage keeping
 * a failed batch for next time and spilling to disk rather than dropping, and
 * the stored chains are verified, also against the external anchors of their heads.
 */
import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, rmSync } from 'node:fs';
import {
  OVERFLOW_FILE,
  anchor,
  audit,
  drain,
  startAnchoring,
  fingerprint,
  history,
  recent,
  verifyStored,
} from '../../../src/platform/audit.ts';
import { getConnection } from '../../../src/platform/storage/index.ts';
import { anchorBucket } from '../support/anchor-bucket.ts';
import {
  AUDIT_FIELD_MAX_CHARS,
  AUDIT_HMAC_CHAIN_PREFIX,
  AUDIT_PENDING_MAX,
  FINGERPRINT_HEX_CHARS,
  MS_PER_SECOND,
  auditAnchorIntervalMs,
} from '../../../src/platform/constants.ts';

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

  it('takes the client address from the last x-forwarded-for hop, the ingress’s, else the socket', () => {
    const forwarded = audit({ action: 'a', req: { headers: { 'x-forwarded-for': ' 1.2.3.4 , 10.0.0.1 ' } } });
    const direct = audit({ action: 'a', req: { headers: {}, socket: { remoteAddress: '5.6.7.8' } } });
    assert.equal(forwarded.ip, '10.0.0.1');
    assert.equal(direct.ip, '5.6.7.8');
  });

  it('records the credential, member and role the request authenticated as, beside the key fingerprint', () => {
    const principal = { credentialId: 'cred-1', memberUser: 'user-1', role: 'operator' };
    const row = audit({ action: 'a', actorKey: 'project-key', req: { headers: {}, principal } });
    assert.deepEqual(
      [row.actor, row.credential_id, row.member_user, row.actor_role],
      [fingerprint('project-key'), 'cred-1', 'user-1', 'operator'],
    );
  });

  it('leaves the principal fields null when the request had no project credential', () => {
    const row = audit({ action: 'a', req: { headers: {} } });
    assert.deepEqual([row.credential_id, row.member_user, row.actor_role], [null, null, null]);
  });

  it('links rows on a keyed (HMAC) chain', () => {
    assert.ok(audit({ action: 'a' }).chain.startsWith(AUDIT_HMAC_CHAIN_PREFIX));
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

describe('verifyStored', () => {
  it('passes untouched stored chains and records the verdict as an event', async () => {
    audit({ action: 'verify.ok' });
    await drain();
    const verdict = await verifyStored();
    assert.equal(verdict.ok, true);
    assert.equal(recent({ action: 'audit.verify' })[0].outcome, 'ok');
  });

  it('names the first stored row that was edited, loudly', async () => {
    audit({ action: 'verify.tamper', targetId: 'before' });
    await drain();
    const [row] = await storedRows('verify.tamper');
    const select = mock.method(getConnection(), 'select', async () => [{ ...row, target_id: 'after' }]);
    const error = mock.method(console, 'error', () => {});
    const verdict = await verifyStored();
    select.mock.restore();
    error.mock.restore();
    assert.deepEqual([verdict.ok, verdict.broken.seq], [false, row.seq]);
    assert.match(error.mock.calls[0].arguments[0], /CHAIN BROKEN/);
    assert.equal(recent({ action: 'audit.verify' })[0].outcome, 'error');
  });
});

describe('anchoring', () => {
  it('is off without an anchor bucket', async () => {
    assert.equal(await anchor(), null);
    assert.equal(startAnchoring(), null);
  });

  it('anchors the newest stored row on drain, and again only once the chain has moved', async () => {
    const bucket = anchorBucket();
    audit({ action: 'anchor.first' });
    await drain(bucket);
    const [row] = await storedRows('anchor.first');
    assert.equal(bucket.objects.size, 1);
    assert.match([...bucket.objects.keys()][0], new RegExp(`/${row.chain}_0*${row.seq}_${row.hash}_`));
    assert.equal(await anchor(bucket), null, 'an unchanged head is not anchored again');
    audit({ action: 'anchor.second' });
    await drain(bucket);
    assert.equal(bucket.objects.size, 2);
  });

  it('never anchors a row storage has not accepted', async () => {
    const bucket = anchorBucket();
    const error = mock.method(console, 'error', () => {});
    const upsert = mock.method(getConnection(), 'upsert', async () => Promise.reject(new Error('storage down')));
    audit({ action: 'anchor.unstored' });
    await drain(bucket);
    upsert.mock.restore();
    error.mock.restore();
    assert.equal(bucket.objects.size, 0);
    await drain(null);
  });

  it('says so, without failing the drain, when the bucket refuses an anchor', async () => {
    const error = mock.method(console, 'error', () => {});
    audit({ action: 'anchor.refused' });
    await drain(anchorBucket('denied'));
    error.mock.restore();
    assert.match(error.mock.calls[0].arguments[0], /anchor not written: denied/);
  });

  it('anchors on the interval', async () => {
    const bucket = anchorBucket();
    audit({ action: 'anchor.timer' });
    await drain(null);
    mock.timers.enable({ apis: ['setInterval'] });
    const timer = startAnchoring(bucket);
    mock.timers.tick(auditAnchorIntervalMs());
    await new Promise((resolve) => setImmediate(resolve));
    clearInterval(timer);
    mock.timers.reset();
    assert.equal(bucket.objects.size, 1);
  });

  it('verification passes when every anchored row is stored as anchored', async () => {
    const bucket = anchorBucket();
    audit({ action: 'anchor.verify' });
    await drain(bucket);
    const verdict = await verifyStored(bucket);
    assert.deepEqual([verdict.ok, verdict.anchors], [true, 1]);
  });

  it('verification reports rows cut from the end of a chain, which still links up', async () => {
    const bucket = anchorBucket();
    audit({ action: 'anchor.cut' });
    await drain(bucket);
    const [row] = await storedRows('anchor.cut');
    const connection = getConnection();
    const real = connection.select.bind(connection);
    const select = mock.method(connection, 'select', async (...args: [string, any?, any?]) =>
      (await real(...args)).filter((r) => r.seq !== row.seq || r.chain !== row.chain),
    );
    const error = mock.method(console, 'error', () => {});
    const verdict = await verifyStored(bucket);
    select.mock.restore();
    error.mock.restore();
    assert.deepEqual([verdict.ok, verdict.broken.seq], [false, row.seq]);
    assert.match(verdict.broken.reason, /anchored row is missing/);
  });

  it('verification reports an unreadable anchor bucket beside the verdict, not as a break', async () => {
    const warn = mock.method(console, 'warn', () => {});
    const verdict = await verifyStored(anchorBucket('offline'));
    warn.mock.restore();
    assert.deepEqual([verdict.ok, verdict.anchorError], [true, 'audit anchors unreadable: offline']);
    assert.match(warn.mock.calls.at(-1).arguments[0], /anchors not compared/);
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

  it('spills events past the backlog bound to the overflow file instead of dropping them', async () => {
    rmSync(OVERFLOW_FILE, { force: true });
    const error = mock.method(console, 'error', () => {});
    const upsert = mock.method(getConnection(), 'upsert', async () => Promise.reject(new Error('storage down')));
    for (let i = 0; i <= AUDIT_PENDING_MAX; i++) audit({ action: 'overflow.test', targetId: String(i) });
    const spilled = readFileSync(OVERFLOW_FILE, 'utf8')
      .trim()
      .split('\n')
      .map((line) => JSON.parse(line));
    assert.deepEqual(
      spilled.map((r) => r.target_id),
      [String(AUDIT_PENDING_MAX)],
    );
    assert.ok(spilled[0].hash, 'a spilled row keeps its chain link');
    assert.match(error.mock.calls.at(-1).arguments[0], /BACKLOG FULL/);
    upsert.mock.restore();
    error.mock.restore();
    await drain();
  });
});
