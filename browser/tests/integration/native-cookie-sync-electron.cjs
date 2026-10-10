/** Real native cookie storage preserves stronger local logins while authentication accepts other imported cookies. */
const assert = require('node:assert/strict');
const { app, session } = require('electron');
if (!process.env.OYA_COOKIE_PROFILE || !process.env.OYA_COOKIE_BUNDLE) throw Error('Use the isolated launcher');
app.setPath('userData', process.env.OYA_COOKIE_PROFILE);
const { CookieSync } = require(process.env.OYA_COOKIE_BUNDLE);
/** Exercise the production sync with real native rejection codes and no external network or user profile. */
async function run() {
  await app.whenReady();
  const jar = session.fromPartition('cookie-conflicts');
  assert.equal(typeof jar._getOyaSessionPolicy, 'function', 'Requires Oya native engine');
  await jar.cookies.set({ url: 'https://cookie.test/', name: 'secure', value: 'local-secure', secure: true });
  await jar.cookies.set({ url: 'https://cookie.test/', name: 'httpOnly', value: 'local-http', httpOnly: true });
  let mark = 100;
  const sync = new CookieSync({
    session: () => jar,
    ready: () => true,
    open: () => true,
    send: () => true,
    mark: {
      get: () => mark,
      set: (value) => {
        mark = value;
      },
    },
  });
  const imported = (name) => ({ name, value: 'server-copy', domain: 'cookie.test', path: '/', t: 200 });
  await sync.applyCookieSync(['secure', 'httpOnly', 'normal'].map(imported), { now: 200 });
  const saved = Object.fromEntries((await jar.cookies.get({})).map((cookie) => [cookie.name, cookie]));
  assert.equal(saved.secure.value, 'local-secure');
  assert.equal(saved.secure.secure, true);
  assert.equal(saved.httpOnly.value, 'local-http');
  assert.equal(saved.httpOnly.httpOnly, true);
  assert.equal(saved.normal.value, 'server-copy');
  assert.equal(mark, 200);
  await assert.rejects(
    sync.applyCookieSync([{ ...imported('invalid'), sameSite: 'None', secure: false }], { now: 300 }),
    /EXCLUDE_SAMESITE_NONE_INSECURE/,
  );
  assert.equal(mark, 200, 'invalid cookie failures remain fatal without advancing freshness');
  console.log('PASS native Secure/HttpOnly overwrite preservation and invalid-cookie refusal');
}
run()
  .then(() => app.exit(0))
  .catch((error) => {
    console.error(error);
    app.exit(1);
  });
