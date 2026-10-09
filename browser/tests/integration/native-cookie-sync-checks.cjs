/** Real native cookie-store rejection must not turn a partially restored login into a successful sync. */
const assert = require('node:assert/strict');
const { CookieSync } = require('../../src/main/sync/cookie-sync.ts');
/** Use only the disposable Oya session; no account cookies or debugging transport enter this regression. */
module.exports = async function cookieSyncChecks(session) {
  let stamp = 100;
  const sync = new CookieSync({
    session: () => session,
    send() {},
    open: () => true,
    ready: () => true,
    mark: {
      get: () => stamp,
      set: (value) => {
        stamp = value;
      },
    },
  });
  const good = { name: 'native_sync_test', value: 'synthetic', domain: '127.0.0.1', path: '/', httpOnly: true };
  const bad = { ...good, name: 'invalid\nname', value: 'private-fixture-value' };
  try {
    await assert.rejects(sync.applyCookieSync([good, bad], { now: 200 }), (error) => {
      assert.match(error.message, /Cookie sync incomplete/);
      assert.doesNotMatch(error.message, /private-fixture-value/);
      return true;
    });
    assert.equal(stamp, 100, 'partial native rejection must not advance the sync mark');
    const saved = await session.cookies.get({ name: good.name });
    assert.equal(saved[0].value, good.value);
    assert.equal(saved[0].httpOnly, true);
    await sync.applyCookieSync([{ ...good, name: 'native_sync_retry' }], { now: 300 });
    assert.equal(stamp, 300, 'a successful retry advances freshness');
    assert.equal((await session.cookies.get({ name: 'native_sync_retry' }))[0].value, good.value);
  } finally {
    await session.cookies.remove('http://127.0.0.1/', good.name);
    await session.cookies.remove('http://127.0.0.1/', 'native_sync_retry');
  }
};
