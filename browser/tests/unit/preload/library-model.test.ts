/** Search the whole library literally, with separate sorting and local-calendar grouping. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { matching, dayLabel, sites, fileSize } from '../../../src/preload/library-model.ts';
const pages = [
  { url: 'https://docs.test/api', title: 'API Guide', time: 10 },
  { url: 'https://notes.test/guide', title: 'Travel notes', time: 20 },
  { url: 'https://docs.test/setup', title: 'Getting started', time: 30 },
];
it('finds words across titles and URLs without treating regex characters as patterns', () => {
  assert.deepEqual(matching(pages, ' api DOCS ', '', 'recent'), [pages[0]]);
  assert.deepEqual(matching(pages, '.*', '', 'recent'), []);
});
it('combines a site filter with search and sorts without changing saved data', () => {
  assert.deepEqual(matching(pages, 'guide', 'docs.test', 'recent'), [pages[0]]);
  assert.deepEqual(matching(pages, '', '', 'recent'), [pages[2], pages[1], pages[0]]);
  assert.deepEqual(matching(pages, '', '', 'title'), [pages[0], pages[2], pages[1]]);
  assert.equal(pages[0].time, 10);
  assert.deepEqual(sites(pages), [
    ['docs.test', 2],
    ['notes.test', 1],
  ]);
  assert.deepEqual(
    sites([
      ...pages,
      { url: 'data:', title: 'inline.txt', time: 40 },
      { url: 'blob:https://a.test/1', title: 'b', time: 50 },
    ]),
    [
      ['docs.test', 2],
      ['notes.test', 1],
    ],
  );
});
it('groups history by local calendar days rather than elapsed 24-hour periods', () => {
  const now = new Date(2026, 10, 2, 0, 30);
  assert.equal(dayLabel(new Date(2026, 10, 2).getTime(), now), 'Today');
  assert.equal(dayLabel(new Date(2026, 10, 1, 0, 1).getTime(), now), 'Yesterday');
  assert.notEqual(dayLabel(new Date(2026, 9, 31).getTime(), now), 'Yesterday');
});

it('reads a byte count the way people do', () => {
  assert.deepEqual([0, 512, 3_500, 3_565_158, 12 * 1024 ** 3].map(fileSize), [
    '0 B',
    '512 B',
    '3.4 KB',
    '3.4 MB',
    '12 GB',
  ]);
});
