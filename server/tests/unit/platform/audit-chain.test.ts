/**
 * Unit tests for audit tamper evidence: each row links to the one before it by
 * a keyed hash, a changed or removed row is found and named, a jsonb round trip
 * does not break a hash, two chains are verified independently, a window of the
 * newest rows verifies from its first row, and unkeyed chains are counted.
 */
import { createHash } from 'node:crypto';
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { canonical, head, link, linkHash, verifyAll, verifyChain } from '../../../src/platform/audit-chain.ts';
import { AUDIT_GENESIS_HASH, AUDIT_HMAC_CHAIN_PREFIX } from '../../../src/platform/constants.ts';

/** A row as audit() builds them, before linking. */
const row = (over = {}) => ({
  ts: '2026-09-20T12:00:00.000Z',
  action: 'browser.provision',
  actor: 'abcd1234',
  actor_user: null,
  target_type: 'browser',
  target_id: 'b-1',
  outcome: 'ok',
  ip: '10.0.0.1',
  user_agent: 'agent/1',
  meta: { region: 'us-east-1', phi: false },
  ...over,
});

describe('link', () => {
  it('starts a chain at the genesis hash and counts from one', () => {
    const first = link(row());
    assert.equal(first.seq >= 1, true);
    if (first.seq === 1) assert.equal(first.prev_hash, AUDIT_GENESIS_HASH);
    assert.match(first.hash, /^[0-9a-f]{64}$/);
  });

  it('links each row to the hash of the one before it', () => {
    const a = link(row());
    const b = link(row({ action: 'key.create' }));
    assert.equal(b.prev_hash, a.hash);
    assert.equal(b.seq, a.seq + 1);
    assert.equal(b.chain, a.chain);
  });

  it('signs with a key, so the link cannot be recomputed as a plain SHA-256', () => {
    const placed = link(row());
    const plain = createHash('sha256')
      .update(`${placed.prev_hash}\n${canonical(placed)}`)
      .digest('hex');
    assert.ok(placed.chain.startsWith(AUDIT_HMAC_CHAIN_PREFIX));
    assert.notEqual(placed.hash, plain);
  });

  it('signs who authenticated, so changing the credential, member or role breaks the link', () => {
    const placed = link(row({ credential_id: 'c-1', member_user: 'u-1', actor_role: 'viewer' }));
    for (const field of ['credential_id', 'member_user', 'actor_role'])
      assert.notEqual(linkHash(placed.prev_hash, { ...placed, [field]: 'x' }), placed.hash);
  });

  it('reports its head so a report can anchor the chain', () => {
    const last = link(row());
    assert.deepEqual(head(), { chain: last.chain, seq: last.seq, hash: last.hash });
  });
});

describe('canonical', () => {
  it('does not change when a meta key order changes, as jsonb reorders them', () => {
    const a = canonical(link(row({ meta: { a: 1, b: { c: 2, d: 3 } } })));
    const reordered = canonical({ ...JSON.parse(JSON.stringify({ ...row({ meta: { b: { d: 3, c: 2 }, a: 1 } }) })) });
    assert.equal(a.includes('"a":1'), true);
    assert.equal(reordered.includes('"a":1'), true);
  });

  it('reads one instant from either timestamp spelling', () => {
    const zulu = link(row({ ts: '2026-09-20T12:00:00.000Z' }));
    const offset = { ...zulu, ts: '2026-09-20T12:00:00+00:00' };
    assert.equal(zulu.hash, linkHash(zulu.prev_hash, offset));
  });

  it('cannot be fooled by moving text from one field to the next', () => {
    const a = link(row({ action: 'ab', actor: 'c' }));
    const b = { ...a, action: 'a', actor: 'bc' };
    assert.notEqual(a.hash, linkHash(a.prev_hash, b));
  });
});

describe('verifyChain', () => {
  it('accepts an untouched run of rows', () => {
    const rows = [link(row()), link(row()), link(row())].map((r, i) => ({ ...r, seq: i + 1, chain: 'c' }));
    const relinked = relink(rows);
    assert.deepEqual(verifyChain(relinked), { ok: true, checked: 3 });
  });

  it('names the row whose contents were edited', () => {
    const rows = relink([link(row()), link(row()), link(row())].map((r, i) => ({ ...r, seq: i + 1, chain: 'c' })));
    rows[1] = { ...rows[1], outcome: 'denied' };
    const verdict = verifyChain(rows);
    assert.equal(verdict.ok, false);
    assert.equal(verdict.broken.seq, 2);
    assert.match(verdict.broken.reason, /hash does not match/);
  });

  it('notices a row that was removed', () => {
    const rows = relink([link(row()), link(row()), link(row())].map((r, i) => ({ ...r, seq: i + 1, chain: 'c' })));
    const verdict = verifyChain([rows[0], rows[2]]);
    assert.equal(verdict.ok, false);
    assert.match(verdict.broken.reason, /missing or reordered/);
  });
});

describe('verifyAll', () => {
  it('verifies each instance chain from its own genesis', () => {
    const one = relink([link(row()), link(row())].map((r, i) => ({ ...r, seq: i + 1, chain: 'one' })));
    const two = relink([link(row()), link(row())].map((r, i) => ({ ...r, seq: i + 1, chain: 'two' })));
    const verdict = verifyAll([...two, ...one]);
    assert.deepEqual(verdict, { ok: true, checked: 4, chains: 2, unkeyed: 4 });
  });

  it('counts no unkeyed rows on keyed chains', () => {
    const keyed = `${AUDIT_HMAC_CHAIN_PREFIX}k`;
    const rows = relink([link(row()), link(row())].map((r, i) => ({ ...r, seq: i + 1, chain: keyed })));
    assert.deepEqual(verifyAll(rows), { ok: true, checked: 2, chains: 1, unkeyed: 0 });
  });

  it('verifies a window of the newest rows from its first row when anchored, and refuses it otherwise', () => {
    const rows = relink([link(row()), link(row()), link(row())].map((r, i) => ({ ...r, seq: i + 1, chain: 'w' })));
    assert.equal(verifyAll(rows.slice(1), true).ok, true);
    assert.equal(verifyAll(rows.slice(1)).ok, false);
  });

  it('still finds an edit inside an anchored window', () => {
    const rows = relink([link(row()), link(row()), link(row())].map((r, i) => ({ ...r, seq: i + 1, chain: 'w' })));
    const verdict = verifyAll([rows[1], { ...rows[2], outcome: 'denied' }], true);
    assert.deepEqual([verdict.ok, verdict.broken.seq], [false, 3]);
  });

  it('ignores rows written before the chain existed', () => {
    const legacy = [{ ...row(), seq: null, chain: null, prev_hash: null, hash: null }];
    assert.equal(verifyAll(legacy).ok, true);
  });
});

/** Recomputes a run's links so a fixture reads like rows one writer produced. */
function relink(rows) {
  let prev = AUDIT_GENESIS_HASH;
  return rows.map((r) => {
    const placed = { ...r, prev_hash: prev };
    placed.hash = linkHash(prev, placed);
    prev = placed.hash;
    return placed;
  });
}

describe('chain key changes', () => {
  it('still verifies chains signed under the derived key after OYA_AUDIT_HMAC_KEY is set', () => {
    const before = process.env.OYA_AUDIT_HMAC_KEY;
    delete process.env.OYA_AUDIT_HMAC_KEY;
    const row = link({ ts: 1, action: 'key.change.before' });
    process.env.OYA_AUDIT_HMAC_KEY = 'an-operator-key-set-later';
    try {
      const after = link({ ts: 2, action: 'key.change.after' });
      assert.equal(verifyAll([row, after], true).ok, true);
    } finally {
      if (before === undefined) delete process.env.OYA_AUDIT_HMAC_KEY;
      else process.env.OYA_AUDIT_HMAC_KEY = before;
    }
  });
});
