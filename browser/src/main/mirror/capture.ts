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
import { CdpWs } from './cdp-ws.ts';
import { readVersion, readCookies, type SlimCookie } from './read.ts';
import { copyIfPresent } from './copy.ts';
import { LAUNCH_READY_TIMEOUT_MS, LAUNCH_POLL_MS, LAUNCH_INPUTS, PROFILE_INPUTS, PROFILE_STORES } from './constants.ts';

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

/** The debugging port the launched browser wrote, or null until it has. */
function readPort(scratch: string): number | null {
  try {
    return Number(fs.readFileSync(path.join(scratch, 'DevToolsActivePort'), 'utf8').split('\n')[0]) || null;
  } catch {
    return null;
  }
}

/** Waits for the launched browser to advertise its debugging port. */
async function waitForPort(scratch: string): Promise<number> {
  const deadline = Date.now() + LAUNCH_READY_TIMEOUT_MS;
  do {
    const port = readPort(scratch);
    if (port) return port;
    await sleep(LAUNCH_POLL_MS);
  } while (Date.now() < deadline);
  throw new Error('Launched browser never opened its debugging port');
}

/** Resolves after `ms`, without blocking a timer from letting the process exit. */
function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms).unref?.());
}

/** Snapshot, launch, read the profile's cookies and the real version, then tear down. */
async function captureProfile(scratchRoot: string, source: ChromiumSource, profile: SourceProfile) {
  const scratch = fs.mkdtempSync(path.join(scratchRoot, 'oya-mirror-'));
  snapshot(source, profile.dir, scratch);
  const child = launch(source.exe, scratch, profile.dir);
  try {
    return await readOverCdp(await waitForPort(scratch), profile);
  } finally {
    teardown(child, scratch);
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

/** Kills the launched browser and deletes its scratch copy. */
function teardown(child: ChildProcess, scratch: string): void {
  try {
    child.kill();
  } catch {}
  fs.rmSync(scratch, { recursive: true, force: true });
}

/** The whole capture: the real device once, then each profile's cookies. Null when nothing to mirror. */
export async function captureAll(ctx: CaptureDeps, sourceId?: string): Promise<Capture | null> {
  const source = sourceBrowser(sourceId);
  if (!source) return null;
  const device = await captureDevice(ctx.electron);
  // Firefox's cookies are read from SQLite; Chromium needs a launch per profile
  // to decrypt them. The persona presents this app's own engine version either
  // way (identity/identity.ts): claiming the source browser's is a detectable lie.
  const profiles = source.kind === 'firefox' ? source.capture() : await captureProfiles(source);
  if (!profiles.length) return null;
  return { source: source.id, name: source.name, userDataDir: source.userDataDir, device, profiles };
}

/** Each profile captured in turn; a profile that fails is logged and skipped, not fatal. */
async function captureProfiles(source: ChromiumSource): Promise<ProfileCapture[]> {
  const scratchRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-mirror-root-'));
  const out: ProfileCapture[] = [];
  try {
    for (const p of source.profiles) await pushCapture(out, scratchRoot, source, p);
  } finally {
    fs.rmSync(scratchRoot, { recursive: true, force: true });
  }
  return out;
}

/** Captures one profile into `out`; a profile that fails is logged and skipped, never fatal. */
async function pushCapture(out: ProfileCapture[], scratchRoot: string, source: ChromiumSource, p: SourceProfile) {
  try {
    out.push(await captureProfile(scratchRoot, source, p));
  } catch (e) {
    console.log(`[oya] Mirror skipped profile ${p.dir}: ${(e as Error).message}`);
  }
}
