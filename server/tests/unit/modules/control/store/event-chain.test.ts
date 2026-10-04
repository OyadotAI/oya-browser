/**
 * Unit tests for the control event chain: linked events verify, an edited
 * event and one removed from the middle are named, and a pruned start is not
 * a break.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { eventHash, verifyEventChain } from '../../../../../src/modules/control/store/event-chain.ts';
import { EVENT_GENESIS_HASH } from '../../../../../src/modules/control/store/constants.ts';

/** Four linked events of one project, as stored. */
function chain() {
  const rows = [];
  let prev = EVENT_GENESIS_HASH;
  for (const [i, type] of ['a', 'b', 'c', 'd'].entries()) {
    const row = { seq: i + 1, project: 'p1', type, session_id: null, at: i, detail: '{"n":1}', prev_hash: prev };
    prev = eventHash(prev, row);
    rows.push({ ...row, hash: prev });
  }
  return rows;
}

describe('verifyEventChain', () => {
  it('passes linked events', () => {
    assert.deepEqual(verifyEventChain(chain()), { ok: true, checked: 4 });
  });

  it('names an edited event', () => {
    const rows = chain();
    rows[2].detail = '{"n":2}';
    assert.deepEqual(verifyEventChain(rows), {
      ok: false,
      checked: 2,
      broken: { seq: 3, reason: 'hash does not match the event, it was edited' },
    });
  });

  it('names the event after one removed from the middle', () => {
    const rows = chain();
    rows.splice(1, 1);
    assert.equal(verifyEventChain(rows).broken.seq, 3);
  });

  it('accepts a start pruned by retention', () => {
    assert.deepEqual(verifyEventChain(chain().slice(2)), { ok: true, checked: 2 });
  });

  it('hashes a missing session as empty text', () => {
    const row = { project: 'p', type: 't', at: 1, detail: '{}' };
    assert.equal(eventHash('x', { ...row, session_id: null }), eventHash('x', { ...row, session_id: '' }));
  });
});
