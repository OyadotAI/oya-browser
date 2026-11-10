/**
 * The popup's rules: what it shows on opening, and when it reads and sends a
 * site's cookies (only with a key, a granted site and a key that may write).
 */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { openPopup, saveKey, sendLogins } from '../../src/popup-model.js';
import { Messages } from '../../src/messages.js';
import { fakeBrowser, fakeFetch, offlineFetch } from './support/fakes.ts';

const PROFILES = {
  status: 200,
  body: {
    personas: [
      { id: 'p1', name: 'Work' },
      { id: 'p2', name: 'Home' },
    ],
  },
};
const MEMBER = { status: 200, body: { role: 'member' } };
const COOKIE = { name: 'sid', value: 'abc', domain: '.example.com', path: '/', secure: true, httpOnly: true };
const SITE = { url: 'https://mail.example.com/inbox', host: 'mail.example.com', pattern: '*://*.example.com/*' };
const request = { key: 'k', site: SITE, profile: { id: 'p1', name: 'Work' } };

test('without a key the popup asks for one and calls nothing', async () => {
  const { fetchFn, requests } = fakeFetch({});
  const opened = await openPopup(fakeBrowser().api, fetchFn);
  assert.equal(opened.message, Messages.noKey);
  assert.equal(requests.length, 0);
});

test('a page that is not a website has nothing to send', async () => {
  const { api } = fakeBrowser({ url: 'chrome://settings', stored: { apiKey: 'k' } });
  assert.equal((await openPopup(api, fakeFetch({}).fetchFn)).message, Messages.notWebsite);
});

test('a key with no profiles points to the console', async () => {
  const { fetchFn } = fakeFetch({ 'GET /api/personas': { status: 200, body: { personas: [] } } });
  const opened = await openPopup(fakeBrowser({ stored: { apiKey: 'k' } }).api, fetchFn);
  assert.equal(opened.message, Messages.noProfiles);
});

test('a refused key on opening says so', async () => {
  const { fetchFn } = fakeFetch({ 'GET /api/personas': { status: 401, body: { error: 'expired' } } });
  const opened = await openPopup(fakeBrowser({ stored: { apiKey: 'k' } }).api, fetchFn);
  assert.equal(opened.message, Messages.expired);
});

test('the last profile used is preselected', async () => {
  const { api } = fakeBrowser({ stored: { apiKey: 'k', lastProfile: 'p2' }, granted: true });
  const opened = await openPopup(api, fakeFetch({ 'GET /api/personas': PROFILES }).fetchFn);
  assert.equal(opened.ready, true);
  assert.equal(opened.selected, 'p2');
  assert.equal(opened.site.pattern, '*://*.example.com/*');
});

test('denied site access reads nothing and sends nothing', async () => {
  const { api, asked } = fakeBrowser({ allows: false, cookies: [COOKIE] });
  const { fetchFn, requests } = fakeFetch({});
  assert.equal(await sendLogins(api, fetchFn, request), Messages.denied);
  assert.equal(asked.cookieQueries.length, 0);
  assert.equal(requests.length, 0);
});

test('access is asked for the whole site, so parent-domain logins are readable', async () => {
  const { api, asked } = fakeBrowser({ cookies: [COOKIE] });
  const { fetchFn } = fakeFetch({ 'GET /api/auth/whoami': MEMBER, 'PUT /api/pool/cookies': { status: 200, body: {} } });
  await sendLogins(api, fetchFn, request);
  assert.deepEqual(asked.permissionRequests, [{ origins: ['*://*.example.com/*'] }]);
});

test('a viewer key is stopped before any cookie is read', async () => {
  const { api, asked } = fakeBrowser({ cookies: [COOKIE] });
  const { fetchFn } = fakeFetch({ 'GET /api/auth/whoami': { status: 200, body: { role: 'viewer' } } });
  assert.equal(await sendLogins(api, fetchFn, request), Messages.viewer);
  assert.equal(asked.cookieQueries.length, 0);
});

test('a site with no cookies sends nothing', async () => {
  const { fetchFn, requests } = fakeFetch({ 'GET /api/auth/whoami': MEMBER });
  assert.equal(await sendLogins(fakeBrowser().api, fetchFn, request), Messages.nothing);
  assert.equal(requests.filter((r) => r.method === 'PUT').length, 0);
});

test('sending puts the cookies in the chosen profile and remembers it', async () => {
  const { api, stored } = fakeBrowser({ cookies: [COOKIE] });
  const imported = { status: 200, body: { imported: 1, skipped: 0, total: 1 } };
  const { fetchFn, requests } = fakeFetch({ 'GET /api/auth/whoami': MEMBER, 'PUT /api/pool/cookies': imported });
  const message = await sendLogins(api, fetchFn, request);
  const put = requests.find((r) => r.method === 'PUT');
  assert.equal(put?.url, 'https://oyabrowser.com/api/pool/cookies?persona=p1');
  assert.equal((put?.body as { cookies: unknown[] }).cookies.length, 1);
  assert.match(message.text, /^1 login sent to Work\./);
  assert.equal(stored.lastProfile, 'p1');
});

test('Oya unreachable while sending says nothing was sent', async () => {
  const { api } = fakeBrowser({ cookies: [COOKIE] });
  assert.equal(await sendLogins(api, offlineFetch, request), Messages.offline);
});

test('a key is kept only when Oya accepts it', async () => {
  const { api, stored } = fakeBrowser();
  await assert.rejects(saveKey(api, fakeFetch({ 'GET /api/personas': { status: 403 } }).fetchFn, 'bad'));
  assert.equal(stored.apiKey, undefined);
  assert.equal(await saveKey(api, fakeFetch({ 'GET /api/personas': PROFILES }).fetchFn, 'good'), 2);
  assert.equal(stored.apiKey, 'good');
});

test('Firefox without access yet warns that its prompt may close the popup', async () => {
  const { api } = fakeBrowser({ stored: { apiKey: 'k' }, origin: 'moz-extension://abc/' });
  const opened = await openPopup(api, fakeFetch({ 'GET /api/personas': PROFILES }).fetchFn);
  assert.equal(opened.hint, Messages.askAccess);
});

test('Chrome, or a site already allowed, shows no access hint', async () => {
  const { fetchFn } = fakeFetch({ 'GET /api/personas': PROFILES });
  assert.equal((await openPopup(fakeBrowser({ stored: { apiKey: 'k' } }).api, fetchFn)).hint, null);
  const allowed = fakeBrowser({ stored: { apiKey: 'k' }, origin: 'moz-extension://abc/', granted: true });
  assert.equal((await openPopup(allowed.api, fetchFn)).hint, null);
});
