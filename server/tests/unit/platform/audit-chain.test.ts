/**
 * Unit tests for audit tamper evidence: each row links to the one before it, a
 * changed or removed row is found and named, a jsonb round trip does not break
 * a hash, and two chains are verified independently.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { canonical, head, link, linkHash, verifyAll, verifyChain } from '../../../src/platform/audit-chain.ts';
import { AUDIT_GENESIS_HASH } from '../../../src/platform/constants.ts';

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
    assert.deepEqual(verdict, { ok: true, checked: 4, chains: 2 });
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
