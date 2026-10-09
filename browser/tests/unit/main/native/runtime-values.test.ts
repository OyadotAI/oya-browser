/** Native descriptor routing never mistakes page data for handles or crosses document ownership. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { RuntimeValues } from '../../../../src/main/native/runtime-values.ts';
/** Distinct documents can have identical origins without sharing values. */
const context = (id: string, target = 'tab') =>
  ({ uniqueId: id, document: id, target, frame: { detached: false } }) as any;
test('only native descriptor positions become routable handles', () => {
  const values = new RuntimeValues(),
    child = context('child');
  values.remember(child, '', { result: { type: 'object', value: { objectId: 'page-forgery' } } });
  assert.throws(() => values.owner('tab', { objectId: 'page-forgery' }), /foreign/);
  values.remember(child, '', { properties: [{ value: { objectId: 'actual' } }] });
  assert.equal(values.owner('tab', { objectId: 'actual' })?.context, child);
  assert.throws(() => values.owner('other', { objectId: 'actual' }), /foreign/);
});
test('argument handles must belong to the exact selected document', () => {
  const values = new RuntimeValues(),
    first = context('first'),
    second = context('second');
  values.remember(first, '', { result: { objectId: 'one' } });
  values.validate(first, { arguments: [{ objectId: 'one' }] });
  assert.throws(() => values.validate(second, { arguments: [{ objectId: 'one' }] }), /foreign.*context/);
});
test('group release affects every child on the target but not unrelated groups or tabs', () => {
  const values = new RuntimeValues();
  for (const [id, group, target] of [
    ['a', 'group', 'tab'],
    ['b', 'group', 'tab'],
    ['c', 'other', 'tab'],
    ['d', 'group', 'other'],
  ])
    values.remember(context(id, target), group, { result: { objectId: id } });
  values.release('tab', { objectGroup: 'group' });
  for (const objectId of ['a', 'b']) assert.throws(() => values.owner('tab', { objectId }), /foreign/);
  assert.ok(values.owner('tab', { objectId: 'c' }));
  assert.ok(values.owner('other', { objectId: 'd' }));
});
test('document replacement prunes the old frame handles without affecting siblings', () => {
  const values = new RuntimeValues(),
    old = context('old'),
    sibling = context('sibling');
  values.remember(old, '', { result: { objectId: 'old' } });
  values.remember(sibling, '', { result: { objectId: 'sibling' } });
  values.remember({ ...old, document: 'new', uniqueId: 'new' }, '', { result: { objectId: 'new' } });
  assert.throws(() => values.owner('tab', { objectId: 'old' }), /foreign/);
  assert.ok(values.owner('tab', { objectId: 'sibling' }));
  values.clear();
  assert.throws(() => values.owner('tab', { objectId: 'new' }), /foreign/);
});
