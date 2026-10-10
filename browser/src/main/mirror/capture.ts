/** Import browser profiles through offline readers; never launch another browser or attach a debugger. */
import { sourceBrowser, type Source } from './locate.ts';
import { captureDevice, type DeviceElectron } from './device.ts';
import type { SlimCookie } from './read.ts';

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

/** Native import cannot decrypt another Chromium browser's platform-bound cookies. */
export function requireOfflineSource(source: Source): void {
  if (source.kind === 'chromium')
    throw new Error(
      `Import from ${source.name} is unavailable: native cookie decryption is unsupported. Sign in directly in Oya Browser.`,
    );
}

/** Read supported offline profiles without starting a source browser or a debugging transport. */
export async function captureAll(ctx: CaptureDeps, sourceId?: string): Promise<Capture | null> {
  const source = sourceBrowser(sourceId);
  if (!source) return null;
  requireOfflineSource(source);
  if (source.kind !== 'firefox') throw new Error('Unsupported native profile source');
  const device = await captureDevice(ctx.electron);
  const profiles = source.capture();
  if (!profiles.length) return null;
  return { source: source.id, name: source.name, userDataDir: source.userDataDir, device, profiles, warnings: [] };
}

/** Preserve the existing import result contract for callers reporting partial offline captures. */
export function capturedOrError(profiles: ProfileCapture[], warnings: string[]): ProfileCapture[] {
  if (!profiles.length && warnings.length) throw new Error(warnings.join('\n'));
  return profiles;
}
