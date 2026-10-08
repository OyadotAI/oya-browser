/** Resolve installed Windows browsers without assuming a per-user installation. */
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

/** Registry roots checked for application registrations, in preference order. */
const ROOTS = ['HKCU', 'HKLM'];
/** Registry view paths used by native and 32-bit installers. */
const APP_PATHS = [
  'Software\\Microsoft\\Windows\\CurrentVersion\\App Paths',
  'Software\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\App Paths',
];

/** Reads a registered application path without expanding or executing its contents. */
function registered(key: string): string {
  try {
    const output = execFileSync('reg', ['query', key, '/ve'], { encoding: 'utf8' });
    return registryValue(output);
  } catch {
    return '';
  }
}

/** Extract the registry's string value without retaining display prefixes. */
function registryValue(output: string): string {
  const value = output.match(/REG_(?:EXPAND_)?SZ\s+(.+)/)?.[1] || '';
  return value.trim().replace(/^"|"$/g, '');
}

/** Resolve seams allow Windows installation layouts to be tested on every OS. */
export interface WindowsLocations {
  /** Windows environment, including installation roots. */
  env: Record<string, string | undefined>;
  /** Reads the default application registration. */
  registry(key: string): string;
  /** Confirms a candidate executable exists. */
  exists(file: string): boolean;
}

/** Expands Windows environment references in registry values. */
function expand(value: string, env: WindowsLocations['env']): string {
  return value.replace(
    /%([^%]+)%/g,
    (_, key: string) => Object.entries(env).find(([name]) => name.toLowerCase() === key.toLowerCase())?.[1] || '',
  );
}

/** Registry candidates come before standard installation locations. */
function executableCandidates(relative: string, deps: WindowsLocations): string[] {
  const binary = path.win32.basename(relative);
  const keys = ROOTS.flatMap((root) => APP_PATHS.map((part) => `${root}\\${part}\\${binary}`));
  const registeredPaths = keys.map((key) => expand(deps.registry(key), deps.env));
  const roots = [deps.env.LOCALAPPDATA, deps.env.ProgramFiles, deps.env['ProgramFiles(x86)']];
  return [...registeredPaths, ...roots.filter(Boolean).map((root) => path.win32.join(root!, relative))];
}

/** Finds an executable in App Paths, then standard user and machine installation roots. */
export function windowsExecutable(
  relative: string,
  deps: WindowsLocations = { env: process.env, registry: registered, exists: fs.existsSync },
): string {
  const binary = path.win32.basename(relative);
  const candidates = executableCandidates(relative, deps);
  const found = candidates.find((file) => file && deps.exists(file));
  if (!found) throw new Error(`Could not find ${binary}. Install the browser or repair its installation, then retry.`);
  return found;
}
