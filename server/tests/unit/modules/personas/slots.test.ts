/**
 * Unit tests for PersonaSlots: the per-persona concurrency cap, refused past
 * the cap and reported to metrics as browsers come and go.
 */
import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { PersonaSlots } from '../../../../src/modules/personas/slots.ts';
import { shape } from '../../../../src/modules/personas/model.ts';
import { DEFAULT_MAX_CONCURRENT } from '../../../../src/modules/personas/constants.ts';
import { Status } from '../../../../src/platform/http-status.ts';

/** Slots over recording metrics. */
function slots() {
  const metrics = { personaCapped: { inc: mock.fn() }, personasActive: { set: mock.fn() } };
  return { slots: new PersonaSlots(metrics), metrics };
}
/** A persona with this cap. */
const persona = (id: string, maxConcurrent?: number | null) => shape({ id, name: `P ${id}`, maxConcurrent });

describe('PersonaSlots', () => {
  it('takes slots up to the cap and refuses the next with a 429', () => {
    const { slots: s, metrics } = slots();
    const p = persona('a', 2);
    s.acquire(p, 'b1');
    s.acquire(p, 'b2');
    assert.throws(() => s.acquire(p, 'b3'), {
      status: Status.TOO_MANY_REQUESTS,
      message: 'Persona "P a" already has 2 of 2 browsers running',
    });
    assert.equal(metrics.personaCapped.inc.mock.callCount(), 1);
    assert.equal(s.count('a'), 2);
  });

  it('applies the default cap to a persona without one', () => {
    const { slots: s } = slots();
    const p = { ...persona('a'), maxConcurrent: undefined } as any;
    for (let i = 0; i < DEFAULT_MAX_CONCURRENT; i++) s.acquire(p, `b${i}`);
    assert.throws(() => s.acquire(p, 'one-more'), { status: Status.TOO_MANY_REQUESTS });
  });

  it('never refuses an uncapped persona', () => {
    const { slots: s } = slots();
    const p = persona('a', null);
    for (let i = 0; i < 50; i++) s.acquire(p, `b${i}`);
    assert.equal(s.count('a'), 50);
  });

  it('counts a browser once however often it acquires', () => {
    const { slots: s } = slots();
    const p = persona('a', 1);
    s.acquire(p, 'b1');
    s.acquire(p, 'b1');
    assert.equal(s.count('a'), 1);
  });

  it('keeps each persona’s slots separate', () => {
    const { slots: s } = slots();
    s.acquire(persona('a', 1), 'b1');
    assert.doesNotThrow(() => s.acquire(persona('b', 1), 'b2'));
  });

  it('frees a slot on release and reports the new total', () => {
    const { slots: s, metrics } = slots();
    const p = persona('a', 1);
    s.acquire(p, 'b1');
    s.release(p, 'b1');
    assert.equal(s.count('a'), 0);
    assert.deepEqual(metrics.personasActive.set.mock.calls.at(-1).arguments, [{}, 0]);
  });

  it('ignores a release for no persona or one with nothing running', () => {
    const { slots: s, metrics } = slots();
    s.release(null, 'b1');
    s.release(persona('a'), 'b1');
    assert.equal(metrics.personasActive.set.mock.callCount(), 0);
  });

  it('reports the total across personas', () => {
    const { slots: s, metrics } = slots();
    s.acquire(persona('a'), 'b1');
    s.acquire(persona('b'), 'b2');
    s.report();
    assert.deepEqual(metrics.personasActive.set.mock.calls.at(-1).arguments, [{}, 2]);
  });

  it('forgets one persona on delete, and all of them on clear', () => {
    const { slots: s } = slots();
    s.acquire(persona('a'), 'b1');
    s.acquire(persona('b'), 'b2');
    s.delete('a');
    assert.deepEqual([s.count('a'), s.count('b')], [0, 1]);
    s.clear();
    assert.equal(s.count('b'), 0);
  });
});
