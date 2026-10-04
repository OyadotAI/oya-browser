/**
 * Where the user's real browser lives, per OS: which browser is the default,
 * its executable, its user-data directory, and the profiles inside it. This is
 * the only OS-specific part of the mirror; everything downstream reads the
 * profile the same way, because the browser itself does the decrypting.
 */
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { firefoxSource, type FirefoxSource } from './firefox.ts';

/** A profile a Chromium browser holds, as Local State lists it. */
export interface SourceProfile {
  /** The profile's directory name inside the browser's data directory. */
  dir: string;
  /** The name the person gave it. */
  name: string;
  /** Whether it is the profile the browser last opened. */
  lastUsed: boolean;
}

/** A located Chromium browser, ready to capture by launching it. */
export interface ChromiumSource {
  /** The stable source id. */
  id: string;
  /** The display name. */
  name: string;
  /** Tells capture to launch the browser per profile. */
  kind: 'chromium';
  /** The browser's executable. */
  exe: string;
  /** Its user-data directory. */
  userDataDir: string;
  /** The profiles inside it. */
  profiles: SourceProfile[];
}

/** A browser an import can read. */
export type Source = ChromiumSource | FirefoxSource;

/** A browser on this machine, as the import picker lists it. */
export interface ListedSource {
  /** The stable source id. */
  id: string;
  /** The display name. */
  name: string;
  /** Whether it is the person's https default. */
  isDefault: boolean;
  /** Lets the shell read it as a payload (src/shared/ipc.ts). */
  [key: string]: unknown;
}

/** Where a supported browser keeps its app and data on macOS. */
interface MacSpec {
  /** The display name. */
  name: string;
  /** The app bundle's name under /Applications. */
  app: string;
  /** The data directory under Application Support. */
  data: string;
}

/** Where a supported browser keeps its executable and data on Windows. */
interface WinSpec {
  /** The display name. */
  name: string;
  /** The data directory under %LOCALAPPDATA%. */
  data: string;
  /** The executable under %LOCALAPPDATA%. */
  exe: string;
}

/** The macOS bundle id and Windows ProgId marker of Firefox, which is read differently (firefox.ts). */
const FIREFOX_BUNDLE = 'org.mozilla.firefox';

/** A supported browser family: its display name, macOS bundle id, and app/data locations. */
const MAC_BROWSERS: Record<string, MacSpec> = {
  'com.google.chrome': { name: 'Chrome', app: 'Google Chrome', data: 'Google/Chrome' },
  'company.thebrowser.browser': { name: 'Arc', app: 'Arc', data: 'Arc/User Data' },
  'com.brave.browser': { name: 'Brave', app: 'Brave Browser', data: 'BraveSoftware/Brave-Browser' },
  'com.microsoft.edgemac': { name: 'Edge', app: 'Microsoft Edge', data: 'Microsoft Edge' },
};

/** The Windows user-data directory of each family, under %LOCALAPPDATA%. */
const WIN_BROWSERS: Record<string, WinSpec> = {
  chrome: { name: 'Chrome', data: 'Google\\Chrome\\User Data', exe: 'Google\\Chrome\\Application\\chrome.exe' },
  brave: {
    name: 'Brave',
    data: 'BraveSoftware\\Brave-Browser\\User Data',
    exe: 'BraveSoftware\\Brave-Browser\\Application\\brave.exe',
  },
  edge: { name: 'Edge', data: 'Microsoft\\Edge\\User Data', exe: 'Microsoft\\Edge\\Application\\msedge.exe' },
};

/** The macOS bundle id that handles https, or null when it cannot be read. */
function macDefaultBundleId(): string | null {
  try {
    const domain = 'com.apple.LaunchServices/com.apple.launchservices.secure';
    const raw = execFileSync('defaults', ['read', domain, 'LSHandlers'], { encoding: 'utf8' });
    return bundleForHttps(raw);
  } catch {
    return null;
  }
}

/** The `LSHandlerRoleAll` bundle id from the handler block that claims the https scheme. */
export function bundleForHttps(raw: string): string | null {
  for (const block of raw.split('}')) {
    if (/LSHandlerURLScheme\s*=\s*https;/.test(block))
      return block.match(/LSHandlerRoleAll\s*=\s*"?([\w.]+)"?/)?.[1] || null;
  }
  return null;
}

/** A located Chromium browser: its stable id, name, executable, data dir and profiles. */
function describeSource(id: string, name: string, exe: string, userDataDir: string): ChromiumSource {
  return { id, name, kind: 'chromium', exe, userDataDir, profiles: profilesOf(userDataDir) };
}

/** Where macOS apps keep their data. */
const macSupport = (): string => path.join(os.homedir(), 'Library', 'Application Support');

/** Every supported browser with data on this Mac, as `{ id, name }`, the https default first. */
function macSources(): ListedSource[] {
  const bundle = macDefaultBundleId();
  const chromium = Object.entries(MAC_BROWSERS)
    .filter(([, spec]) => fs.existsSync(path.join(macSupport(), spec.data)))
    .map(([bundleId, spec]) => ({ id: macId(spec), name: spec.name, isDefault: bundleId === bundle }));
  const firefox = firefoxSource() ? [{ id: 'firefox', name: 'Firefox', isDefault: bundle === FIREFOX_BUNDLE }] : [];
  return defaultFirst([...chromium, ...firefox]);
}

/** The macOS browser named `id`, ready to capture; null when it is not installed. */
function macSource(id: string): Source | null {
  if (id === 'firefox') return firefoxSource();
  const spec = Object.values(MAC_BROWSERS).find((b) => macId(b) === id);
  if (!spec || !fs.existsSync(path.join(macSupport(), spec.data))) return null;
  return describeSource(id, spec.name, macExe(spec.app), path.join(macSupport(), spec.data));
}

/** The list with the person's default browser first, the rest as they were. */
export function defaultFirst(sources: ListedSource[]): ListedSource[] {
  return [...sources.filter((s) => s.isDefault), ...sources.filter((s) => !s.isDefault)];
}

/** The launcher binary inside a macOS app bundle. */
function macExe(app: string): string {
  return `/Applications/${app}.app/Contents/MacOS/${app}`;
}

/** A stable source key from a browser's macOS app name (lowercased, spaceless). */
function macId(spec: MacSpec): string {
  return spec.name.toLowerCase();
}

/** Every supported browser with data on this Windows machine, as `{ id, name }`, the https default first. */
function winSources(): ListedSource[] {
  const local = process.env.LOCALAPPDATA || '';
  const key = winDefaultKey();
  const chromium = Object.entries(WIN_BROWSERS)
    .filter(([, spec]) => local && fs.existsSync(path.join(local, spec.data)))
    .map(([family, spec]) => ({ id: winId(spec), name: spec.name, isDefault: family === key }));
  const firefox = firefoxSource() ? [{ id: 'firefox', name: 'Firefox', isDefault: key === 'firefox' }] : [];
  return defaultFirst([...chromium, ...firefox]);
}

/** The Windows browser named `id`, ready to capture; null when it is not installed. */
function winSource(id: string): Source | null {
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
function winDefaultKey(): string | null {
  try {
    const raw = execFileSync('reg', ['query', HTTPS_USER_CHOICE, '/v', 'ProgId'], { encoding: 'utf8' });
    const family = ['firefox', 'brave', 'edge', 'chrome'].find((f) => new RegExp(f, 'i').test(raw));
    return family || null;
  } catch {
    return null;
  }
}

/** A stable source key from a Windows browser spec. */
function winId(spec: WinSpec): string {
  return spec.name.toLowerCase();
}

/** Every profile in a user-data dir, from Local State, last-used flagged; a plain Default if unreadable. */
export function profilesOf(userDataDir: string): SourceProfile[] {
  const info = readInfoCache(userDataDir);
  if (!info) return [{ dir: 'Default', name: 'Default', lastUsed: true }];
  const last = info.lastUsed;
  return Object.entries(info.cache).map(([dir, meta]) => ({ dir, name: meta?.name || dir, lastUsed: dir === last }));
}

/** What Local State says of one profile. */
interface ProfileMeta {
  /** The name the person gave it. */
  name?: string;
}

/** Local State's profile section: each profile's metadata, and which was used last. */
interface InfoCache {
  /** Profile directory to its metadata. */
  cache: Record<string, ProfileMeta | null>;
  /** The last-used profile's directory. */
  lastUsed: string;
}

/** The profile info cache and last-used profile from Local State, or null when absent. */
function readInfoCache(userDataDir: string): InfoCache | null {
  try {
    const state = JSON.parse(fs.readFileSync(path.join(userDataDir, 'Local State'), 'utf8'));
    const cache = state.profile?.info_cache;
    return cache && Object.keys(cache).length ? { cache, lastUsed: state.profile?.last_used || 'Default' } : null;
  } catch {
    return null;
  }
}

/** How one OS lists its installed browsers and opens one for capture. */
interface Platform {
  /** The installed browsers, the default first. */
  list: () => ListedSource[];
  /** The browser named `id`, ready to capture. */
  open: (id: string) => Source | null;
}

/** Per OS: how to list the installed browsers, and how to open one for capture. */
const SOURCES: Record<string, Platform> = {
  darwin: { list: macSources, open: macSource },
  win32: { list: winSources, open: winSource },
};

/** The browsers an import can read on this machine, the person's default first; empty on an unsupported platform. */
export function installedSources(): ListedSource[] {
  return Object.hasOwn(SOURCES, process.platform) ? SOURCES[process.platform].list() : [];
}

/** The browser to mirror: the one named `id`, else the person's default (the first listed); null when there is none. */
export function sourceBrowser(id?: string): Source | null {
  const chosen = id || installedSources()[0]?.id;
  return chosen ? SOURCES[process.platform].open(chosen) : null;
}
