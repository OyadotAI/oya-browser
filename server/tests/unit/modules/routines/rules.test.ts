/**
 * Unit tests for a routine's playbook values and profile (routines/rules.ts):
 * values only with a playbook, bounded, dropped with it; a profile id or the
 * default.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { definitionFrom } from '../../../../src/modules/routines/rules.ts';

const PLAY = { name: 'Search', playbook: 'search', schedule: { kind: 'every', n: 1, unit: 'hours' } };

describe('routine values and profile', () => {
  it('keeps a playbook routine’s values, and none for an empty map', () => {
    assert.deepEqual(definitionFrom({ ...PLAY, values: { q: 'mug' } }).values, { q: 'mug' });
    assert.equal(definitionFrom({ ...PLAY, values: {} }).values, undefined);
  });

  it('keeps values on an edit of the same playbook, and drops them when it plays another', () => {
    const current = definitionFrom({ ...PLAY, values: { q: 'x' } });
    assert.deepEqual(definitionFrom({ name: 'renamed' }, current).values, { q: 'x' });
    assert.equal(definitionFrom({ playbook: 'other' }, current).values, undefined);
  });

  it('refuses values on a prompt routine, and drops them when an edit switches back to a prompt', () => {
    assert.throws(() => definitionFrom({ ...PLAY, playbook: undefined, prompt: 'go', values: { q: 'x' } }), {
      status: 400,
    });
    const edited = definitionFrom({ playbook: null, prompt: 'go' }, definitionFrom({ ...PLAY, values: { q: 'x' } }));
    assert.equal(edited.values, undefined);
  });

  it('refuses values that are not a map of variable names to short text', () => {
    assert.throws(() => definitionFrom({ ...PLAY, values: ['x'] }), { status: 400 });
    assert.throws(() => definitionFrom({ ...PLAY, values: { 'bad name': 'x' } }), { status: 400 });
    assert.throws(() => definitionFrom({ ...PLAY, values: { q: 5 } }), { status: 400 });
    assert.throws(() => definitionFrom({ ...PLAY, values: { q: 'x'.repeat(2001) } }), { status: 400 });
    const many = Object.fromEntries(Array.from({ length: 51 }, (_, i) => [`v${i}`, 'x']));
    assert.throws(() => definitionFrom({ ...PLAY, values: many }), { status: 400 });
  });

  it('keeps an edit’s values unless it sends new ones', () => {
    const saved = definitionFrom({ ...PLAY, values: { q: 'mug' } });
    assert.deepEqual(definitionFrom({ name: 'Renamed' }, saved).values, { q: 'mug' });
    assert.deepEqual(definitionFrom({ values: { q: 'cup' } }, saved).values, { q: 'cup' });
  });

  it('names a profile by id, or the default with null, "" or "default"', () => {
    assert.equal(definitionFrom({ ...PLAY, persona: 'p-1' }).persona, 'p-1');
    for (const persona of [null, '', 'default']) assert.equal(definitionFrom({ ...PLAY, persona }).persona, undefined);
    assert.throws(() => definitionFrom({ ...PLAY, persona: 7 }), { status: 400 });
  });
});
