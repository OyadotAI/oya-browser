/**
 * Unit tests for main/cookie-sync.cjs: the dump, applying a server jar, pulls
 * per host with their TTL and timeout, and batched local changes. The session
 * and the control socket are fakes; timers are mocked.
 */
const { describe, it, beforeEach, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { EventEmitter } = require('node:events');
const { createCookieSync, cookieSyncMark } = require('../../../main/cookie-sync.cjs');
const { flush } = require('../support/fakes.cjs');
const {
  COOKIE_PULL_TTL_MS,
  COOKIE_PULL_TIMEOUT_MS,
  COOKIE_FLUSH_MS,
  COOKIE_BATCH_MAX,
} = require('../../../main/constants.cjs');

/** A cookie store: an emitter that lists `jar` and records set() calls. */
function fakeCookies(jar = []) {
  const store = new EventEmitter();
  store.get = async () => jar;
  store.set = async (cookie) => {
    if (cookie.name === 'bad') throw new Error('rejected');
    store.written.push(cookie);
  };
  store.written = [];
  return store;
}

/** A sync over a fake session, with the socket up unless told otherwise. */
function syncWith({ jar, link = { open: true, ready: true }, sendResult = true, syncedAt = 0 } = {}) {
  const sent = [];
  const cookies = fakeCookies(jar);
  const mark = { value: syncedAt, get: () => mark.value, set: (n) => (mark.value = n) };
  const sync = createCookieSync({
    mark,
    session: () => ({ cookies }),
    send: (message) => {
      sent.push(message);
      return sendResult;
    },
    open: () => link.open,
    ready: () => link.ready,
  });
  return { sync, sent, cookies, link, mark };
}

/** A cookie as Electron reports it. */
const cookie = (name, extra = {}) => ({ name, value: 'v', domain: '.x.test', path: '/', secure: true, ...extra });

describe('cookieSyncMark', () => {
  it("keeps each persona's mark in the app config, so it outlives a restart", () => {
    const ctx = { config: { values: {}, save: mock.fn() }, persona: { active: { id: 'p-1' } } };
    const mark = cookieSyncMark(ctx);
    assert.equal(mark.get(), 0);
    mark.set(9000);
    ctx.persona.active = { id: 'p-2' };
    assert.equal(mark.get(), 0, 'another persona has synced nothing yet');
    ctx.persona.active = { id: 'p-1' };
    assert.equal(mark.get(), 9000);
    assert.equal(ctx.config.save.mock.callCount(), 1);
  });
});

describe('createCookieSync', () => {
  beforeEach(() => {
    mock.timers.enable({ apis: ['setTimeout', 'Date'], now: 1_000_000 });
    mock.method(console, 'log', () => {});
  });
  afterEach(() => {
    mock.timers.reset();
    mock.restoreAll();
  });

  it('dumps the jar with the fields the server keeps', async () => {
    const { sync, sent } = syncWith({ jar: [cookie('a', { hostOnly: false, session: true })] });
    await sync.dumpCookies();
    assert.deepEqual(sent, [
      {
        type: 'cookie_dump',
        cookies: [
          {
            name: 'a',
            value: 'v',
            domain: '.x.test',
            path: '/',
            secure: true,
            httpOnly: undefined,
            sameSite: 'unspecified',
            expirationDate: undefined,
            hostOnly: false,
          },
        ],
      },
    ]);
  });

  it('dumps nothing while the socket is down', async () => {
    const { sync, sent } = syncWith({ jar: [cookie('a')], link: { open: false, ready: true } });
    await sync.dumpCookies();
    assert.equal(sent.length, 0);
  });

  it('applies a server jar in Electron spelling, host-only cookies by URL', async () => {
    const { sync, cookies } = syncWith();
    await sync.applyCookieSync([
      { name: 'dom', value: '1', domain: '.x.test', sameSite: 'Lax', expires: 100 },
      { name: 'host', value: '2', domain: 'y.test', path: '/p', secure: true, hostOnly: true, expirationDate: -1 },
    ]);
    assert.deepEqual(cookies.written, [
      {
        url: 'http://x.test/',
        name: 'dom',
        value: '1',
        domain: '.x.test',
        path: '/',
        secure: false,
        httpOnly: false,
        sameSite: 'lax',
        expirationDate: 100,
      },
      {
        url: 'https://y.test/p',
        name: 'host',
        value: '2',
        path: '/p',
        secure: true,
        httpOnly: false,
        sameSite: 'unspecified',
        expirationDate: undefined,
      },
    ]);
  });

  it('applies the rest of the jar when one cookie is refused', async () => {
    const { sync, cookies } = syncWith();
    await sync.applyCookieSync([cookie('bad'), cookie('good')]);
    assert.deepEqual(
      cookies.written.map((c) => c.name),
      ['good'],
    );
    assert.equal(console.log.mock.calls.at(-1).arguments[0], '[oya] Cookie sync applied: 1/2');
  });

  it('ignores a jar that is not a list', async () => {
    const { sync, cookies } = syncWith();
    await sync.applyCookieSync(undefined);
    assert.equal(cookies.written.length, 0);
  });

  it('pulls a host before navigating and resolves on the server answer', async () => {
    const { sync, sent } = syncWith();
    let done = false;
    sync.pullCookiesFor('shop.test/cart').then(() => (done = true));
    assert.deepEqual(sent, [{ type: 'cookie_pull', domains: ['shop.test'], pullId: 'p1' }]);
    sync.answerPull('p1');
    await flush();
    assert.equal(done, true);
  });

  it('does not pull the same host again within the TTL, unless forced', async () => {
    const { sync, sent } = syncWith();
    sync.pullCookiesFor('https://a.test/');
    sync.answerPull('p1');
    await sync.pullCookiesFor('https://a.test/other');
    assert.equal(sent.length, 1);
    sync.pullCookiesFor('https://a.test/', { force: true });
    assert.equal(sent.length, 2);
    mock.timers.tick(COOKIE_PULL_TTL_MS);
    sync.pullCookiesFor('https://a.test/');
    assert.equal(sent.length, 3);
  });

  it('never blocks a navigation past the timeout, and retries a host that timed out', async () => {
    const { sync, sent } = syncWith();
    let done = false;
    sync.pullCookiesFor('https://slow.test/').then(() => (done = true));
    mock.timers.tick(COOKIE_PULL_TIMEOUT_MS);
    await flush();
    assert.equal(done, true);
    sync.pullCookiesFor('https://slow.test/');
    assert.equal(sent.length, 2, 'a timed-out pull does not suppress the next one');
  });

  it('resolves at once when the pull cannot be sent', async () => {
    const { sync } = syncWith({ sendResult: false });
    await sync.pullCookiesFor('https://a.test/');
  });

  it('skips pulls while disconnected or for an unparseable address', async () => {
    const { sync, sent, link } = syncWith();
    await sync.pullCookiesFor('http://[bad');
    link.ready = false;
    await sync.pullCookiesFor('https://a.test/');
    assert.equal(sent.length, 0);
  });

  it('forgets pulled hosts when the persona changes', () => {
    const { sync, sent } = syncWith();
    sync.pullCookiesFor('https://a.test/');
    sync.answerPull('p1');
    sync.forgetPulls();
    sync.pullCookiesFor('https://a.test/');
    assert.equal(sent.length, 2);
  });

  it('batches explicit local changes, keeping only the last value of each cookie', () => {
    const { sync, sent, cookies } = syncWith();
    sync.startCookieChangeListener();
    cookies.emit('changed', {}, cookie('a', { value: '1' }), 'explicit', false);
    cookies.emit('changed', {}, cookie('a', { value: '2' }), 'explicit', false);
    cookies.emit('changed', {}, cookie('b'), 'overwrite', true);
    assert.equal(sent.length, 0);
    mock.timers.tick(COOKIE_FLUSH_MS);
    assert.equal(sent.length, 1);
    assert.deepEqual(
      sent[0].changes.map((c) => [c.cookie.name, c.cookie.value, c.removed]),
      [['a', '2', false]],
    );
  });

  it('sends a full batch at once', () => {
    const { sync, sent, cookies } = syncWith();
    sync.startCookieChangeListener();
    for (let i = 0; i < COOKIE_BATCH_MAX; i++) cookies.emit('changed', {}, cookie('c' + i), 'explicit', false);
    assert.equal(sent.length, 1);
    assert.equal(sent[0].changes.length, COOKIE_BATCH_MAX);
  });

  it('does not echo the cookies it is applying from the server', async () => {
    const { sync, sent, cookies } = syncWith();
    sync.startCookieChangeListener();
    cookies.set = async (c) => cookies.emit('changed', {}, c, 'explicit', false);
    await sync.applyCookieSync([cookie('a')]);
    sync.flushCookieChanges();
    assert.equal(sent.length, 0);
  });

  it('drops queued changes when the persona changes', () => {
    const { sync, sent, cookies } = syncWith();
    sync.startCookieChangeListener();
    cookies.emit('changed', {}, cookie('a'), 'explicit', false);
    sync.dropPendingCookieChanges();
    mock.timers.tick(COOKIE_FLUSH_MS);
    sync.flushCookieChanges();
    assert.equal(sent.length, 0);
  });

  it('keeps a batch that comes due while disconnected, and sends it once back online', () => {
    const { sync, sent, cookies, link } = syncWith();
    sync.startCookieChangeListener();
    cookies.emit('changed', {}, cookie('a'), 'explicit', false);
    link.open = false;
    mock.timers.tick(COOKIE_FLUSH_MS);
    assert.equal(sent.length, 0);
    link.open = true;
    sync.flushCookieChanges();
    assert.deepEqual(
      sent[0].changes.map((c) => c.cookie.name),
      ['a'],
    );
  });

  it('forwards a cookie a site sets, whatever this Electron calls that: "explicit" before 36, "inserted" since', () => {
    const { sync, sent, cookies } = syncWith();
    sync.startCookieChangeListener();
    cookies.emit('changed', {}, cookie('old-name'), 'explicit', false);
    cookies.emit('changed', {}, cookie('new-name'), 'inserted', false);
    cookies.emit('changed', {}, cookie('same-value-new-expiry'), 'inserted-no-value-change-overwrite', false);
    cookies.emit('changed', {}, cookie('untouched'), 'inserted-no-change-overwrite', false);
    mock.timers.tick(COOKIE_FLUSH_MS);
    assert.deepEqual(
      sent[0].changes.map((c) => c.cookie.name),
      ['old-name', 'new-name', 'same-value-new-expiry'],
    );
  });

  it('tells the server when a site deletes a cookie by expiring it, as a logout does', () => {
    const { sync, sent, cookies } = syncWith();
    sync.startCookieChangeListener();
    cookies.emit('changed', {}, cookie('sid'), 'expired-overwrite', true);
    mock.timers.tick(COOKIE_FLUSH_MS);
    assert.deepEqual(
      sent[0].changes.map((c) => [c.cookie.name, c.removed]),
      [['sid', true]],
    );
  });

  it("keeps a fresh login over the server's older copy of the same cookie when it reconnects", async () => {
    const { sync, cookies } = syncWith({ jar: [cookie('sid', { value: 'fresh login' })], syncedAt: 5000 });
    await sync.applyCookieSync([cookie('sid', { value: 'stale', t: 4000 }), cookie('other', { t: 4000 })], {
      now: 9000,
    });
    assert.deepEqual(
      cookies.written.map((c) => c.name),
      ['other'],
      'a cookie this jar lacks is filled in; one it has is left alone unless the server changed it since',
    );
  });

  it('takes a cookie another browser refreshed since this one last synced', async () => {
    const { sync, cookies, mark } = syncWith({ jar: [cookie('sid', { value: 'old' })], syncedAt: 5000 });
    await sync.applyCookieSync([cookie('sid', { value: 'refreshed elsewhere', t: 7000 })], { now: 9000 });
    assert.deepEqual(
      cookies.written.map((c) => c.value),
      ['refreshed elsewhere'],
    );
    assert.equal(mark.value, 9000, 'and is now in step with the server as of its clock');
  });

  it('never lets the server overwrite a local change it has not been sent yet', async () => {
    const { sync, cookies, link } = syncWith({ jar: [cookie('sid')], syncedAt: 5000 });
    sync.startCookieChangeListener();
    link.open = false;
    cookies.emit('changed', {}, cookie('sid', { value: 'logged in offline' }), 'explicit', false);
    await sync.applyCookieSync([cookie('sid', { value: 'server copy', t: 8000 })], { now: 9000 });
    assert.equal(cookies.written.length, 0);
  });

  it('sends its pending changes before it pulls, so the answer cannot be older than this jar', () => {
    const { sync, sent, cookies } = syncWith();
    sync.startCookieChangeListener();
    cookies.emit('changed', {}, cookie('sid'), 'explicit', false);
    sync.pullCookiesFor('https://mail.x.test/');
    assert.deepEqual(
      sent.map((m) => m.type),
      ['cookie_changed', 'cookie_pull'],
    );
  });

  it("judges a pull's answer by when that host was last pulled, without moving the connect mark", async () => {
    const { sync, cookies, mark, sent } = syncWith({ jar: [cookie('sid')], syncedAt: 5000 });
    sync.pullCookiesFor('https://a.x.test/');
    await sync.applyCookieSync([cookie('sid', { value: 'newer', t: 6000 })], { now: 7000, pullId: sent[0].pullId });
    sync.pullCookiesFor('https://a.x.test/', { force: true });
    await sync.applyCookieSync([cookie('sid', { value: 'same as before', t: 6000 })], {
      now: 8000,
      pullId: sent[1].pullId,
    });
    assert.deepEqual(
      cookies.written.map((c) => c.value),
      ['newer'],
    );
    assert.equal(mark.value, 5000);
  });

  it('listens to one cookie store at a time', () => {
    const { sync, cookies } = syncWith();
    sync.startCookieChangeListener();
    sync.startCookieChangeListener();
    assert.equal(cookies.listenerCount('changed'), 1);
  });
});
