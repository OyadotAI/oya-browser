/**
 * Unit tests for audit anchors: a head is sealed and added to the bucket
 * without overwriting, the newest anchor of each chain is found, and comparing
 * anchors with the stored rows names a missing tail, a rewritten row, a missing
 * start and a forged anchor.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { checkAnchors, latestAnchors, writeAnchor } from '../../../src/platform/audit-anchor.ts';
import { anchorBucket } from '../support/anchor-bucket.ts';

const HASH = 'a'.repeat(64);
const OTHER = 'b'.repeat(64);
const DAY = Date.UTC(2026, 9, 4, 12);

/** A row lookup over `rows`, keyed `chain#seq`. */
const rowsAt = (rows: Record<string, any>) => async (chain: string, seq: number) => rows[`${chain}#${seq}`] ?? null;

describe('writeAnchor', () => {
  it('files a sealed head under its day, with everything a check needs in the name', async () => {
    const bucket = anchorBucket();
    const anchor = await writeAnchor(bucket, { chain: 'h-1', seq: 7, hash: HASH }, new Date(DAY));
    const [path] = bucket.objects.keys();
    assert.equal(path, `2026-10-04/h-1_000000000007_${HASH}_${anchor.sig}.json`);
    assert.match(anchor.sig, /^[0-9a-f]{64}$/);
    assert.equal(JSON.parse(bucket.objects.get(path)).at, new Date(DAY).toISOString());
  });

  it('throws when the bucket refuses, including an existing name it may not overwrite', async () => {
    const bucket = anchorBucket();
    const head = { chain: 'h-1', seq: 1, hash: HASH };
    await writeAnchor(bucket, head, new Date(DAY));
    await assert.rejects(writeAnchor(bucket, head, new Date(DAY)), /already exists/);
    await assert.rejects(writeAnchor(anchorBucket('denied'), head), /not written: denied/);
  });
});

describe('latestAnchors', () => {
  it('answers the highest anchor of each chain in the lookback, ignoring other objects', async () => {
    const bucket = anchorBucket();
    await writeAnchor(bucket, { chain: 'h-1', seq: 3, hash: HASH }, new Date(DAY - 86_400_000));
    await writeAnchor(bucket, { chain: 'h-1', seq: 9, hash: OTHER }, new Date(DAY));
    await writeAnchor(bucket, { chain: 'h-2', seq: 1, hash: HASH }, new Date(DAY));
    bucket.objects.set('2026-10-04/readme.txt', 'not an anchor');
    const found = await latestAnchors(bucket, DAY);
    assert.deepEqual(found.map((a) => [a.chain, a.seq]).sort(), [
      ['h-1', 9],
      ['h-2', 1],
    ]);
  });

  it('throws when the bucket cannot be listed', async () => {
    await assert.rejects(latestAnchors(anchorBucket('offline'), DAY), /unreadable: offline/);
  });
});

describe('checkAnchors', () => {
  /** One anchor of chain h-1 at seq 5 with hash HASH. */
  const anchored = async () => [await writeAnchor(anchorBucket(), { chain: 'h-1', seq: 5, hash: HASH })];

  it('passes when the anchored row and the chain start are stored as anchored', async () => {
    const verdict = await checkAnchors(await anchored(), rowsAt({ 'h-1#5': { hash: HASH }, 'h-1#1': { hash: OTHER } }));
    assert.deepEqual(verdict, { ok: true, anchors: 1 });
  });

  it('reports rows cut from the end of a chain', async () => {
    const verdict = await checkAnchors(await anchored(), rowsAt({ 'h-1#1': { hash: OTHER } }));
    assert.equal(verdict.ok, false);
    assert.match(verdict.broken.reason, /anchored row is missing/);
  });

  it('reports a whole chain deleted', async () => {
    const verdict = await checkAnchors(await anchored(), rowsAt({}));
    assert.deepEqual([verdict.broken.chain, verdict.broken.seq], ['h-1', 5]);
  });

  it('reports an anchored row that reads differently', async () => {
    const verdict = await checkAnchors(await anchored(), rowsAt({ 'h-1#5': { hash: OTHER }, 'h-1#1': {} }));
    assert.match(verdict.broken.reason, /rewritten/);
  });

  it('reports rows cut from the start of a chain', async () => {
    const verdict = await checkAnchors(await anchored(), rowsAt({ 'h-1#5': { hash: HASH } }));
    assert.match(verdict.broken.reason, /start of the chain is missing/);
  });

  it('reports an anchor whose seal this deployment did not make', async () => {
    const [anchor] = await anchored();
    const verdict = await checkAnchors([{ ...anchor, sig: OTHER }], rowsAt({ 'h-1#5': { hash: HASH } }));
    assert.match(verdict.broken.reason, /seal does not match/);
  });
});
