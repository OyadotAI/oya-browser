/** Imported profile data must be completely validated before it can reach native storage. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { storageImport, storageOrigin, storageValues } from '../../../../src/main/sync/native-storage-values.ts';
import { STORAGE_ENTRIES_MAX, STORAGE_ORIGINS_MAX, STORAGE_UNITS_MAX } from '../../../../src/main/sync/constants.ts';
it('accepts only exact first-party HTTP origins', () => {
  assert.equal(storageOrigin('https://example.test'), 'https://example.test');
  for (const value of [
    'null',
    'file:///tmp',
    'https://example.test/',
    'https://user@example.test',
    'HTTPS://example.test',
    'https://example.test:443',
  ])
    assert.throws(() => storageOrigin(value), /origin/);
});
it('preserves special property names and exact Unicode code units', () => {
  const values = [
    ['__proto__', 'ordinary'],
    ['constructor', 'value'],
    ['nul\0', '\ud800🗝'],
  ];
  const result = storageValues(values);
  assert.equal(Object.getPrototypeOf(result), Object.prototype);
  assert.deepEqual(Object.entries(result), values);
  assert.deepEqual(storageImport({ 'https://example.test': result }), [['https://example.test', values]]);
});
it('rejects malformed pairs and duplicates instead of coercing values', () => {
  for (const value of [
    null,
    {},
    [['key']],
    [['key', 4]],
    [['key', null]],
    [['key', 'a', 'b']],
    [
      ['key', 'a'],
      ['key', 'b'],
    ],
  ])
    assert.throws(() => storageValues(value as any), /storage/);
});
it('bounds origin count, entry count, and total UTF-16 code units', () => {
  assert.throws(
    () =>
      storageImport(
        Object.fromEntries(Array.from({ length: STORAGE_ORIGINS_MAX + 1 }, (_, i) => [`https://a${i}.test`, {}])),
      ),
    /origins/,
  );
  assert.throws(
    () => storageValues(Array.from({ length: STORAGE_ENTRIES_MAX + 1 }, (_, i) => [String(i), ''])),
    /snapshot/,
  );
  assert.throws(() => storageValues([['key', 'x'.repeat(STORAGE_UNITS_MAX)]]), /limit/);
  assert.equal(storageValues([['', 'x'.repeat(STORAGE_UNITS_MAX)]])[''].length, STORAGE_UNITS_MAX);
});
it('rejects non-dictionary imports and origin values', () => {
  for (const value of [null, [], 'string']) {
    assert.throws(() => storageImport(value as any), /import/);
    assert.throws(() => storageImport({ 'https://example.test': value } as any), /values/);
  }
});
