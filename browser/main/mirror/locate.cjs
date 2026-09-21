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

/** Where macOS apps keep their data. */
const macSupport = () => path.join(os.homedir(), 'Library', 'Application Support');

/** Every supported browser with data on this Mac, as `{ id, name }`, the https default first. */
function macSources() {
  const bundle = macDefaultBundleId();
  const chromium = Object.entries(MAC_BROWSERS)
    .filter(([, spec]) => fs.existsSync(path.join(macSupport(), spec.data)))
    .map(([bundleId, spec]) => ({ id: macId(spec), name: spec.name, isDefault: bundleId === bundle }));
  const firefox = firefoxSource() ? [{ id: 'firefox', name: 'Firefox', isDefault: bundle === FIREFOX_BUNDLE }] : [];
  return defaultFirst([...chromium, ...firefox]);
}

/** The macOS browser named `id`, ready to capture; null when it is not installed. */
function macSource(id) {
  if (id === 'firefox') return firefoxSource();
  const spec = Object.values(MAC_BROWSERS).find((b) => macId(b) === id);
  if (!spec || !fs.existsSync(path.join(macSupport(), spec.data))) return null;
  return describeSource(id, spec.name, macExe(spec.app), path.join(macSupport(), spec.data));
}

/** The list with the person's default browser first, the rest as they were. */
function defaultFirst(sources) {
  return [...sources.filter((s) => s.isDefault), ...sources.filter((s) => !s.isDefault)];
}

/** The launcher binary inside a macOS app bundle. */
function macExe(app) {
  return `/Applications/${app}.app/Contents/MacOS/${app}`;
}

/** A stable source key from a browser's macOS app name (lowercased, spaceless). */
function macId(spec) {
  return spec.name.toLowerCase();
}

/** Every supported browser with data on this Windows machine, as `{ id, name }`, the https default first. */
function winSources() {
  const local = process.env.LOCALAPPDATA || '';
  const key = winDefaultKey();
  const chromium = Object.entries(WIN_BROWSERS)
    .filter(([, spec]) => local && fs.existsSync(path.join(local, spec.data)))
    .map(([family, spec]) => ({ id: winId(spec), name: spec.name, isDefault: family === key }));
  const firefox = firefoxSource() ? [{ id: 'firefox', name: 'Firefox', isDefault: key === 'firefox' }] : [];
  return defaultFirst([...chromium, ...firefox]);
}

/** The Windows browser named `id`, ready to capture; null when it is not installed. */
function winSource(id) {
  if (id === 'firefox') return firefoxSource();
  const local = process.env.LOCALAPPDATA || '';
  const spec = Object.values(WIN_BROWSERS).find((b) => winId(b) === id);
  if (!spec || !local || !fs.existsSync(path.join(local, spec.data))) return null;
  return describeSource(id, spec.name, path.join(local, spec.exe), path.join(local, spec.data));
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

/** Per OS: how to list the installed browsers, and how to open one for capture. */
const SOURCES = { darwin: { list: macSources, open: macSource }, win32: { list: winSources, open: winSource } };

/** The browsers an import can read on this machine, the person's default first; empty on an unsupported platform. */
function installedSources() {
  return Object.hasOwn(SOURCES, process.platform) ? SOURCES[process.platform].list() : [];
}

/** The browser to mirror: the one named `id`, else the person's default (the first listed); null when there is none. */
function sourceBrowser(id) {
  const chosen = id || installedSources()[0]?.id;
  return chosen ? SOURCES[process.platform].open(chosen) : null;
}

module.exports = { sourceBrowser, installedSources, defaultFirst, bundleForHttps, profilesOf };
