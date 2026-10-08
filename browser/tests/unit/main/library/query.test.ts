/** Library queries are literal, bounded, and explicitly paginated. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { queryLibrary } from '../../../../src/main/library/query.ts';
import { QUERY_MAX_LIMIT, QUERY_MAX_LENGTH, BOOKMARK_TITLE_LENGTH } from '../../../../src/main/library/constants.ts';
const entries = [
  { url: 'https://a.test/', title: 'Invoices', time: 2 },
  { url: 'https://b.test/invoices', title: 'Archive', time: 1 },
];
it('searches title and URL literally, and reports the next offset', () => {
  const first = queryLibrary(entries, { query: 'INVOICES', limit: 1 });
  assert.deepEqual(first, { entries: [entries[0]], total: 2, next_offset: 1 });
  assert.deepEqual(queryLibrary(entries, { query: 'invoices', offset: first.next_offset }), {
    entries: [entries[1]],
    total: 2,
    next_offset: null,
  });
  assert.equal(queryLibrary(entries, { query: '.*' }).total, 0);
});
it('rejects invalid query and pagination values instead of coercing them', () => {
  for (const args of [
    { limit: 0 },
    { limit: QUERY_MAX_LIMIT + 1 },
    { limit: '2' },
    { offset: -1 },
    { offset: 0.5 },
    { offset: Infinity },
    { query: {} },
    { query: 'x'.repeat(QUERY_MAX_LENGTH + 1) },
  ])
    assert.throws(() => queryLibrary(entries, args));
});
it('caps titles without modifying storage and permits an exhausted cursor', () => {
  const long = { ...entries[0], title: 'x'.repeat(BOOKMARK_TITLE_LENGTH + 1) };
  assert.equal(queryLibrary([long]).entries[0].title.length, BOOKMARK_TITLE_LENGTH);
  assert.equal(long.title.length, BOOKMARK_TITLE_LENGTH + 1);
  assert.deepEqual(queryLibrary(entries, { offset: 20 }), { entries: [], total: 2, next_offset: null });
});
