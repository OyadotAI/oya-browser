/**
 * Capturing one real browser: snapshot a profile, launch the user's own
 * browser on the copy so it decrypts its own cookies, read them over CDP, and
 * kill it. The copy is why this is safe: Chrome 136+ refuses debugging on the
 * live profile, and we never touch what the user is actually running.
 */
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawn, type ChildProcess } from 'node:child_process';
import { sourceBrowser, type ChromiumSource, type SourceProfile } from './locate.ts';
import { captureDevice, type DeviceElectron } from './device.ts';
import { CaptureProcess } from './capture-process.ts';
import { CdpWs } from './cdp-ws.ts';
import { readVersion, readCookies, type SlimCookie } from './read.ts';
import { copyIfPresent } from './copy.ts';
import { LAUNCH_INPUTS, PROFILE_INPUTS, PROFILE_STORES } from './constants.ts';

/** One captured profile: its identity and its cookies. */
export interface ProfileCapture {
  /** The profile's directory name. */
  profile: string;
  /** The name the person gave it. */
  name: string;
  /** Whether it is the profile the browser last opened. */
  lastUsed: boolean;
  /** The real Chrome version, for a Chromium source. */
  chromeVersion?: string;
  /** Its cookies, in the pool's shape. */
  cookies: SlimCookie[];
}

/** A whole capture: the real device, shared by every profile, and each profile's cookies. */
export interface Capture {
  /** The source browser's id. */
  source: string;
  /** Its display name. */
  name: string;
  /** Its user-data directory, where the site storage is seeded from. */
  userDataDir: string;
  /** The real machine's device, as the hidden window reported it. */
  device: unknown;
  /** Each profile captured. */
  profiles: ProfileCapture[];
  /** Profiles that could not be read, for a partial-import summary. */
  warnings?: string[];
}

/** What the capture takes from the app: Electron, for the device window. */
export interface CaptureDeps {
  /** Electron, whose hidden window reads the real device. */
  electron: DeviceElectron;
}

/** Per-profile files copied so the launched browser can open the profile and its sessions. */
const PROFILE_COPY = [...PROFILE_INPUTS, ...PROFILE_STORES];

/** Makes an offline copy of the profile the launched browser will open. */
function snapshot(source: ChromiumSource, profileDir: string, scratch: string): string {
  for (const input of LAUNCH_INPUTS) copyIfPresent(path.join(source.userDataDir, input), path.join(scratch, input));
  const from = path.join(source.userDataDir, profileDir);
  for (const item of PROFILE_COPY) copyIfPresent(path.join(from, item), path.join(scratch, profileDir, item));
  return path.join(scratch, profileDir);
}

/** The static launch flags: headless, an ephemeral debugging port, and no first-run noise. */
const LAUNCH_FLAGS = [
  '--headless=new',
  '--remote-debugging-port=0',
  '--no-first-run',
  '--no-default-browser-check',
  '--disable-sync',
];

/** Launches the user's browser on the copy, headless, with a debugging port. */
function launch(exe: string, scratch: string, profileDir: string): ChildProcess {
  const args = [`--user-data-dir=${scratch}`, `--profile-directory=${profileDir}`, ...LAUNCH_FLAGS];
  return spawn(exe, args, { stdio: 'ignore' });
}

/** A failed snapshot has no child process and can be removed immediately. */
async function snapshotSafely(source: ChromiumSource, profile: string, scratch: string): Promise<void> {
  try {
    snapshot(source, profile, scratch);
  } catch (error) {
    await fs.promises.rm(scratch, { recursive: true, force: true }).catch(() => {});
    const message = `Could not read ${source.name} profile. Close ${source.name} and retry.`;
    throw new Error(`${message} ${(error as Error).message}`, { cause: error });
  }
}

/** Snapshot, launch, read the profile's cookies and the real version, then tear down. */
async function captureProfile(scratchRoot: string, source: ChromiumSource, profile: SourceProfile) {
  const scratch = fs.mkdtempSync(path.join(scratchRoot, 'oya-mirror-'));
  await snapshotSafely(source, profile.dir, scratch);
  const child = new CaptureProcess(launch(source.exe, scratch, profile.dir));
  try {
    return await readOverCdp(await child.ready(scratch), profile);
  } finally {
    await child.clean(scratch);
  }
}

/** Connects to the launched browser and reads its version and cookies. */
async function readOverCdp(port: number, profile: SourceProfile): Promise<ProfileCapture> {
  const cdp = new CdpWs(port);
  await cdp.connect();
  try {
    return await profileRecord(cdp, profile);
  } finally {
    cdp.close();
  }
}

/** The captured profile: its cookies and the real Chrome version, tagged with the profile's identity. */
async function profileRecord(cdp: CdpWs, profile: SourceProfile): Promise<ProfileCapture> {
  const { chromeVersion } = await readVersion(cdp);
  const cookies = await readCookies(cdp);
  return { profile: profile.dir, name: profile.name, lastUsed: !!profile.lastUsed, chromeVersion, cookies };
}

/** The whole capture: the real device once, then each profile's cookies. Null when nothing to mirror. */
export async function captureAll(ctx: CaptureDeps, sourceId?: string): Promise<Capture | null> {
  const source = sourceBrowser(sourceId);
  if (!source) return null;
  const device = await captureDevice(ctx.electron);
  // Firefox's cookies are read from SQLite; Chromium needs a launch per profile
  // to decrypt them. The persona presents this app's own engine version either
  // way (identity/identity.ts): claiming the source browser's is a detectable lie.
  const warnings: string[] = [];
  const profiles = source.kind === 'firefox' ? source.capture() : await captureProfiles(source, warnings);
  if (!profiles.length) return null;
  return { source: source.id, name: source.name, userDataDir: source.userDataDir, device, profiles, warnings };
}

/** Each profile captured in turn; a profile that fails is logged and skipped, not fatal. */
async function captureProfiles(source: ChromiumSource, warnings: string[]): Promise<ProfileCapture[]> {
  const scratchRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-mirror-root-'));
  const out: ProfileCapture[] = [];
  try {
    for (const p of source.profiles) await pushCapture(out, scratchRoot, source, p, warnings);
  } finally {
    await fs.promises.rmdir(scratchRoot).catch(() => {}); // Retain interrupted captures.
  }
  return capturedOrError(out, warnings);
}

/** Preserve successful profiles but never report all failures as an empty import. */
export function capturedOrError(profiles: ProfileCapture[], warnings: string[]): ProfileCapture[] {
  if (!profiles.length && warnings.length) throw new Error(warnings.join('\n'));
  return profiles;
}

/** Capture collaborators grouped to keep each per-profile operation readable. */
type CaptureArguments = [ProfileCapture[], string, ChromiumSource, SourceProfile, string[]];

/** Captures one profile, retaining its error for a partial-import report. */
async function pushCapture(...[out, scratchRoot, source, p, warnings]: CaptureArguments) {
  try {
    out.push(await captureProfile(scratchRoot, source, p));
  } catch (e) {
    warnings.push(`${p.name}: ${(e as Error).message}`);
  }
}
