/**
 * The Oya release this server belongs to, from the desktop app's package.json: the release script
 * bumps it together with the SDK, so clients can tell whether this server has what they need.
 */
import { readFileSync } from 'node:fs';

/** The part of a package.json read here. */
interface PackageJson {
  /** The release, such as "1.0.128". */
  version?: string;
}

/** The release in browser/package.json; empty when the file is missing (an image built before it was copied). */
function readRelease(): string {
  try {
    const url = new URL('../../../browser/package.json', import.meta.url);
    return (JSON.parse(readFileSync(url, 'utf8')) as PackageJson).version ?? '';
  } catch {
    return '';
  }
}

/** The release, read once at startup. */
export const RELEASE_VERSION = readRelease();
