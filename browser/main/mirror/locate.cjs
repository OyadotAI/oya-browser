/**
 * Where the user's real browser lives, per OS: which browser is the default,
 * its executable, its user-data directory, and the profiles inside it. This is
 * the only OS-specific part of the mirror; everything downstream reads the
 * profile the same way, because the browser itself does the decrypting.
 */
const fs = require('fs');
const os = require('os');
const path = require('path');
const { execFileSync } = require('child_process');
const { firefoxSource } = require('./firefox.cjs');

/** The macOS bundle id and Windows ProgId marker of Firefox, which is read differently (firefox.cjs). */
const FIREFOX_BUNDLE = 'org.mozilla.firefox';

/** A supported browser family: its display name, macOS bundle id, and app/data locations. */
const MAC_BROWSERS = {
  'com.google.chrome': { name: 'Chrome', app: 'Google Chrome', data: 'Google/Chrome' },
  'company.thebrowser.browser': { name: 'Arc', app: 'Arc', data: 'Arc/User Data' },
  'com.brave.browser': { name: 'Brave', app: 'Brave Browser', data: 'BraveSoftware/Brave-Browser' },
  'com.microsoft.edgemac': { name: 'Edge', app: 'Microsoft Edge', data: 'Microsoft Edge' },
};

/** The Windows user-data directory of each family, under %LOCALAPPDATA%. */
const WIN_BROWSERS = {
  chrome: { name: 'Chrome', data: 'Google\\Chrome\\User Data', exe: 'Google\\Chrome\\Application\\chrome.exe' },
  brave: {
    name: 'Brave',
    data: 'BraveSoftware\\Brave-Browser\\User Data',
    exe: 'BraveSoftware\\Brave-Browser\\Application\\brave.exe',
  },
  edge: { name: 'Edge', data: 'Microsoft\\Edge\\User Data', exe: 'Microsoft\\Edge\\Application\\msedge.exe' },
};

/** The macOS bundle id that handles https, or null when it cannot be read. */
function macDefaultBundleId() {
  try {
    const domain = 'com.apple.LaunchServices/com.apple.launchservices.secure';
    const raw = execFileSync('defaults', ['read', domain, 'LSHandlers'], { encoding: 'utf8' });
    return bundleForHttps(raw);
  } catch {
    return null;
  }
}

/** The `LSHandlerRoleAll` bundle id from the handler block that claims the https scheme. */
function bundleForHttps(raw) {
  for (const block of raw.split('}')) {
    if (/LSHandlerURLScheme\s*=\s*https;/.test(block))
      return block.match(/LSHandlerRoleAll\s*=\s*"?([\w.]+)"?/)?.[1] || null;
  }
  return null;
}

/** A located Chromium browser: its stable id, name, executable, data dir and profiles. */
function describeSource(id, name, exe, userDataDir) {
  return { id, name, kind: 'chromium', exe, userDataDir, profiles: profilesOf(userDataDir) };
}

/** The macOS source browser: Firefox when it is the default, else the https default, else the first installed. */
function macSource() {
  const bundle = macDefaultBundleId();
  if (bundle === FIREFOX_BUNDLE) return firefoxSource();
  const support = path.join(os.homedir(), 'Library', 'Application Support');
  const chosen = MAC_BROWSERS[bundle];
  const spec = chosen && fs.existsSync(path.join(support, chosen.data)) ? chosen : firstInstalledMac(support);
  if (!spec) return null;
  return describeSource(macId(spec), spec.name, macExe(spec.app), path.join(support, spec.data));
}

/** The first supported browser with a user-data directory on disk, or null. */
function firstInstalledMac(support) {
  return Object.values(MAC_BROWSERS).find((b) => fs.existsSync(path.join(support, b.data))) || null;
}

/** The launcher binary inside a macOS app bundle. */
function macExe(app) {
  return `/Applications/${app}.app/Contents/MacOS/${app}`;
}

/** A stable source key from a browser's macOS app name (lowercased, spaceless). */
function macId(spec) {
  return spec.name.toLowerCase();
}

/** The Windows source browser: the https default when supported, else the first installed one. */
function winSource() {
  const local = process.env.LOCALAPPDATA;
  if (!local) return null;
  const key = winDefaultKey();
  if (key === 'firefox') return firefoxSource();
  const spec = WIN_BROWSERS[key] || firstInstalledWin(local);
  if (!spec) return null;
  return describeSource(winId(spec), spec.name, path.join(local, spec.exe), path.join(local, spec.data));
}

/** The registry key holding the user's chosen https handler. */
const HTTPS_USER_CHOICE =
  'HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Shell\\Associations\\URLAssociations\\https\\UserChoice';

/** The family key of the registry https handler, or null. */
function winDefaultKey() {
  try {
    const raw = execFileSync('reg', ['query', HTTPS_USER_CHOICE, '/v', 'ProgId'], { encoding: 'utf8' });
    const family = ['firefox', 'brave', 'edge', 'chrome'].find((f) => new RegExp(f, 'i').test(raw));
    return family || null;
  } catch {
    return null;
  }
}

/** The first supported Windows browser with a user-data directory on disk, or null. */
function firstInstalledWin(local) {
  return Object.values(WIN_BROWSERS).find((b) => fs.existsSync(path.join(local, b.data))) || null;
}

/** A stable source key from a Windows browser spec. */
function winId(spec) {
  return spec.name.toLowerCase();
}

/** Every profile in a user-data dir, from Local State, last-used flagged; a plain Default if unreadable. */
function profilesOf(userDataDir) {
  const info = readInfoCache(userDataDir);
  if (!info) return [{ dir: 'Default', name: 'Default', lastUsed: true }];
  const last = info.lastUsed;
  return Object.entries(info.cache).map(([dir, meta]) => ({ dir, name: meta?.name || dir, lastUsed: dir === last }));
}

/** The profile info cache and last-used profile from Local State, or null when absent. */
function readInfoCache(userDataDir) {
  try {
    const state = JSON.parse(fs.readFileSync(path.join(userDataDir, 'Local State'), 'utf8'));
    const cache = state.profile?.info_cache;
    return cache && Object.keys(cache).length ? { cache, lastUsed: state.profile?.last_used || 'Default' } : null;
  } catch {
    return null;
  }
}

/** The OS strategy: the user's real browser and its profiles, or null on an unsupported platform. */
const SOURCES = { darwin: macSource, win32: winSource };

/** The user's default browser to mirror, or null when none is supported or found. */
function sourceBrowser() {
  const strategy = SOURCES[process.platform];
  return strategy ? strategy() : null;
}

module.exports = { sourceBrowser, bundleForHttps, profilesOf };
