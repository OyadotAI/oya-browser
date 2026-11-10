/** Which pages count as a site, and the one host permission that covers a site's logins. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { siteOf } from '../../src/site.js';

test('a subdomain asks for its whole site, where parent-domain logins live', () => {
  assert.equal(siteOf('https://mail.google.com/mail/u/0')?.pattern, '*://*.google.com/*');
});

test('a country second-level suffix is never asked for on its own', () => {
  assert.equal(siteOf('https://www.bbc.co.uk/news')?.pattern, '*://*.bbc.co.uk/*');
  assert.equal(siteOf('https://shop.example.com.au/')?.pattern, '*://*.example.com.au/*');
});

test('an IP address or single-label host asks for that host only', () => {
  assert.equal(siteOf('http://127.0.0.1:3000/')?.pattern, '*://127.0.0.1/*');
  assert.equal(siteOf('http://localhost:8080/')?.pattern, '*://localhost/*');
});

test('a trailing dot is dropped, so the grant never widens to a top-level domain', () => {
  assert.equal(siteOf('https://example.com./')?.pattern, '*://*.example.com/*');
  assert.equal(siteOf('https://com./')?.pattern, '*://com/*');
});

test('pages that are not websites are not sites', () => {
  for (const url of ['chrome://extensions', 'about:blank', 'file:///tmp/a.html', 'moz-extension://x/popup.html', '']) {
    assert.equal(siteOf(url), null, url);
  }
});
