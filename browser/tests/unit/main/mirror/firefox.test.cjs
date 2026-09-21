/**
 * Unit tests for mirror/firefox.cjs: Firefox cookies come back in the pool's
 * slim shape with millisecond expiry converted to seconds, and the profile
 * Firefox actually uses is the install default, not the legacy Default=1 flag.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('fs');
const os = require('os');
const path = require('path');
const { slimFirefox, firefoxProfile } = require('../../../../main/mirror/firefox.cjs');

describe('slimFirefox', () => {
  it('converts millisecond expiry to seconds and maps sameSite and flags', () => {
    const c = slimFirefox({
      name: 's',
      value: 'v',
      host: '.google.com',
      path: '/',
      expiry: 1810307372724,
      isSecure: 1,
      isHttpOnly: 1,
      sameSite: 1,
    });
    assert.deepEqual(c, {
      name: 's',
      value: 'v',
      domain: '.google.com',
      path: '/',
      secure: true,
      httpOnly: true,
      sameSite: 'Lax',
      hostOnly: false,
      expirationDate: 1810307372,
    });
  });

  it('marks a bare host as host-only and leaves an unknown sameSite unspecified', () => {
    const c = slimFirefox({
      name: 'x',
      value: '1',
      host: 'mail.example.com',
      path: '/',
      expiry: 0,
      isSecure: 0,
      isHttpOnly: 0,
      sameSite: 256,
    });
    assert.equal(c.hostOnly, true);
    assert.equal(c.sameSite, 'unspecified');
    assert.equal('expirationDate' in c, false);
  });
});

describe('firefoxProfile', () => {
  /** Writes a profiles.ini into a fresh temp Firefox dir and returns the dir. */
  const withIni = (text) => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-ffp-'));
    fs.writeFileSync(path.join(dir, 'profiles.ini'), text);
    return dir;
  };

  it("uses the install's default profile over the legacy Default=1 flag", () => {
    const dir = withIni(
      '[Profile1]\nPath=Profiles/legacy.default\nDefault=1\n\n[Profile0]\nPath=Profiles/real.default-release\n\n[Install1]\nDefault=Profiles/real.default-release\n',
    );
    assert.equal(firefoxProfile(dir), path.join(dir, 'Profiles/real.default-release'));
  });

  it('falls back to the legacy default when there is no install section', () => {
    const dir = withIni('[Profile0]\nPath=Profiles/only.default\nDefault=1\n');
    assert.equal(firefoxProfile(dir), path.join(dir, 'Profiles/only.default'));
  });
});

describe('readFirefoxCookies', () => {
  const { execFileSync } = require('node:child_process');
  const { readFirefoxCookies } = require('../../../../main/mirror/firefox.cjs');
  const SCHEMA =
    'CREATE TABLE moz_cookies (name TEXT, value TEXT, host TEXT, path TEXT, expiry INTEGER, isSecure INTEGER, isHttpOnly INTEGER, sameSite INTEGER);';
  /** One cookie row. */
  const insert = (name) => `INSERT INTO moz_cookies VALUES ('${name}', 'v', '.site.test', '/', 0, 1, 1, 1);`;

  it('reads the newest logins too: those a running Firefox still holds in its write-ahead log', (t) => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-ff-wal-'));
    t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
    const [live, db] = ['live.sqlite', 'cookies.sqlite'].map((name) => path.join(dir, name));
    // The files are copied while the connection is still open, as they are under a running
    // Firefox: the second row is then only in the -wal file. Closing would fold it into the database.
    const sql = `PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; ${SCHEMA} ${insert('old')} PRAGMA wal_checkpoint(TRUNCATE); ${insert('fresh-login')}`;
    try {
      execFileSync('sqlite3', [live, sql, `.shell cp ${live} ${db}`, `.shell cp ${live}-wal ${db}-wal`], {
        stdio: 'ignore',
      });
    } catch {
      return t.skip('no sqlite3 on this machine');
    }
    const names = readFirefoxCookies(dir).map((c) => c.name);
    assert.deepEqual(names.sort(), ['fresh-login', 'old']);
  });
});
