/** The cookies read for a site, in the shape Oya's jar keeps. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { SLIM_FIELDS, readSiteCookies } from '../../src/site-cookies.js';
import { TAB_ID, fakeBrowser } from './support/fakes.ts';

const TAB = { id: TAB_ID };

const FULL = {
  name: 'sid',
  value: 'abc',
  domain: '.example.com',
  path: '/',
  secure: true,
  httpOnly: true,
  sameSite: 'lax',
  expirationDate: 1900000000,
  hostOnly: false,
  session: false,
  storeId: '0',
};

test('cookies are sent with only the fields the jar keeps', async () => {
  const { api } = fakeBrowser({ cookies: [FULL] });
  const [cookie] = await readSiteCookies(api, 'https://example.com/', TAB);
  assert.deepEqual(Object.keys(cookie), [...SLIM_FIELDS]);
});

test('a cookie with no sameSite is sent as unspecified', async () => {
  const { api } = fakeBrowser({ cookies: [{ ...FULL, sameSite: undefined }] });
  assert.equal((await readSiteCookies(api, 'https://example.com/', TAB))[0].sameSite, 'unspecified');
});

test('partitioned cookies belong to embeds elsewhere and are left out', async () => {
  const { api } = fakeBrowser({ cookies: [FULL, { ...FULL, name: 'embed', partitionKey: { topLevelSite: 'x' } }] });
  assert.deepEqual(
    (await readSiteCookies(api, 'https://example.com/', TAB)).map((c) => c.name),
    ['sid'],
  );
});

test('Chrome reads the store serving the tab, so incognito sends its own logins', async () => {
  const stores = [
    { id: '0', tabIds: [1] },
    { id: '1', tabIds: [TAB_ID] },
  ];
  const { api, asked } = fakeBrowser({ stores });
  await readSiteCookies(api, 'https://example.com/', TAB);
  assert.deepEqual(asked.cookieQueries, [{ url: 'https://example.com/', storeId: '1' }]);
});

test('Firefox reads the tab container across first-party jars', async () => {
  const { api, asked } = fakeBrowser({ origin: 'moz-extension://abc/' });
  await readSiteCookies(api, 'https://example.com/', { id: TAB_ID, cookieStoreId: 'firefox-container-2' });
  const query = { url: 'https://example.com/', storeId: 'firefox-container-2', firstPartyDomain: null };
  assert.deepEqual(asked.cookieQueries, [query]);
});
