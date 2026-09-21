/**
 * Firefox is not Chromium: it cannot be driven over CDP, but it does not need
 * to be. Its cookies live unencrypted in a SQLite file, so we read them
 * directly. Only cookies are imported (they carry the logins); the device is
 * the real machine's, captured the same way as for a Chromium source.
 */
const fs = require('fs');
const os = require('os');
const path = require('path');
const { execFileSync } = require('child_process');
const { copyIfPresent } = require('./copy.cjs');
const { MS_PER_SECOND, SQLITE_MAX_BUFFER } = require('./constants.cjs');

/** Firefox's sameSite numbers; anything else (0, or a newer bitfield) is left unspecified. */
const FF_SAME_SITE = { 1: 'Lax', 2: 'Strict' };
/** The columns read from moz_cookies. */
const COOKIE_SQL = 'SELECT name,value,host,path,expiry,isSecure,isHttpOnly,sameSite FROM moz_cookies';

/** The Firefox data directory for this OS, or null when there is none. */
function firefoxDir() {
  if (process.platform === 'darwin') return path.join(os.homedir(), 'Library', 'Application Support', 'Firefox');
  if (process.platform === 'win32' && process.env.APPDATA) return path.join(process.env.APPDATA, 'Mozilla', 'Firefox');
  return null;
}

/** profiles.ini parsed into `{ section: { key: value } }`. */
function parseIni(text) {
  const out = {};
  let section = '';
  for (const line of text.split(/\r?\n/)) section = iniLine(out, section, line);
  return out;
}

/** Applies one INI line to `out`, returning the section it leaves us in. */
function iniLine(out, section, line) {
  const header = line.match(/^\[(.+)\]$/);
  if (header) return (void (out[header[1]] = {}), header[1]);
  const eq = line.indexOf('=');
  if (section && eq > 0) out[section][line.slice(0, eq)] = line.slice(eq + 1);
  return section;
}

/** The install's chosen profile path, which overrides the legacy default flag. */
function installDefault(ini) {
  const install = Object.keys(ini).find((s) => s.startsWith('Install'));
  return install ? ini[install].Default : null;
}

/** The legacy default profile's relative path, from the section marked Default=1. */
function legacyDefault(ini) {
  const name = Object.keys(ini).find((s) => s.startsWith('Profile') && ini[s].Default === '1');
  return name ? ini[name].Path : null;
}

/** The absolute path of the profile Firefox actually uses, or null. */
function firefoxProfile(dir) {
  try {
    const ini = parseIni(fs.readFileSync(path.join(dir, 'profiles.ini'), 'utf8'));
    const rel = installDefault(ini) || legacyDefault(ini);
    return rel ? path.join(dir, rel) : null;
  } catch {
    return null;
  }
}

/** One Firefox cookie row as the server stores it. */
function slimFirefox(c) {
  return { name: c.name, value: c.value, domain: c.host, path: c.path || '/', ...firefoxFlags(c) };
}

/** A Firefox cookie's flags and expiry in the server's spelling; expiry is milliseconds, stored as seconds. */
function firefoxFlags(c) {
  return {
    secure: !!c.isSecure,
    httpOnly: !!c.isHttpOnly,
    sameSite: FF_SAME_SITE[c.sameSite] || 'unspecified',
    hostOnly: !String(c.host).startsWith('.'),
    ...(c.expiry > 0 ? { expirationDate: Math.floor(c.expiry / MS_PER_SECOND) } : {}),
  };
}

/**
 * Reads the profile's cookies from a copy of its SQLite file, slimmed for the pool.
 * The write-ahead log is copied with it: a running Firefox keeps the newest
 * cookies (the logins of the last hours) there, and without it they were missing.
 */
function readFirefoxCookies(profileDir) {
  const copy = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'oya-ff-')), 'cookies.sqlite');
  for (const suffix of ['', '-wal']) copyIfPresent(path.join(profileDir, `cookies.sqlite${suffix}`), copy + suffix);
  try {
    return queryCookies(copy).map(slimFirefox);
  } finally {
    fs.rmSync(path.dirname(copy), { recursive: true, force: true });
  }
}

/** Every row of a cookies.sqlite file, by the system's sqlite3. */
function queryCookies(file) {
  const opts = { encoding: 'utf8', maxBuffer: SQLITE_MAX_BUFFER };
  return JSON.parse(execFileSync('sqlite3', ['-json', file, COOKIE_SQL], opts) || '[]');
}

/** The profile directory Firefox uses, when it has a cookie jar to read; else null. */
function firefoxCookieProfile() {
  const dir = firefoxDir();
  const profileDir = dir && firefoxProfile(dir);
  return profileDir && fs.existsSync(path.join(profileDir, 'cookies.sqlite')) ? profileDir : null;
}

/**
 * Firefox as a mirror source: its one default profile. Null when unavailable.
 * The cookies are read by `capture`, not here: finding out which browsers are
 * installed used to read the whole jar.
 */
function firefoxSource() {
  const profileDir = firefoxCookieProfile();
  if (!profileDir) return null;
  const profile = { profile: 'default', name: 'default', lastUsed: true };
  const capture = () => [{ ...profile, cookies: readFirefoxCookies(profileDir) }];
  return { id: 'firefox', name: 'Firefox', kind: 'firefox', userDataDir: firefoxDir(), profiles: [profile], capture };
}

module.exports = { firefoxSource, slimFirefox, firefoxProfile, readFirefoxCookies };
