/** Unit tests for scripts/indexnow.mjs: what it reads from a sitemap and what it sends. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { KEY, sitemapUrls, submission } from './indexnow.mjs';

test('reads every <loc> from a sitemap, in order', () => {
  const xml = '<urlset><url><loc>https://a.com/</loc></url><url><loc>\n https://a.com/ai-browser </loc></url></urlset>';
  assert.deepEqual(sitemapUrls(xml), ['https://a.com/', 'https://a.com/ai-browser']);
});

test('sends the host, the key and where the key file lives', () => {
  assert.deepEqual(submission('https://oyabrowser.com', ['https://oyabrowser.com/']), {
    host: 'oyabrowser.com',
    key: KEY,
    keyLocation: `https://oyabrowser.com/${KEY}.txt`,
    urlList: ['https://oyabrowser.com/'],
  });
});

test('the site serves a key file holding exactly the key', () => {
  assert.equal(readFileSync(new URL(`../ui/public/${KEY}.txt`, import.meta.url), 'utf8'), KEY);
});
