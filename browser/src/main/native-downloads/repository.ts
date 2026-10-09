/** Destination repository: never trust a page filename or overwrite an existing filesystem entry. */
import { closeSync, openSync, realpathSync, statSync } from 'node:fs';
import { isAbsolute, join, resolve } from 'node:path';
/** Validate an explicitly requested existing directory, rejecting symlink aliases. */
export function downloadDirectory(path: string): string {
  if (!isAbsolute(path)) throw Error('Download path must be absolute');
  const canonical = realpathSync(path);
  if (canonical !== resolve(path) || !statSync(canonical).isDirectory())
    throw Error('Download path must be an existing non-symlink directory');
  return canonical;
}
/** Reserve an unpredictable GUID filename exclusively before native downloading starts. */
export function reserveDownload(directory: string, guid: string): string {
  downloadDirectory(directory);
  const destination = join(directory, guid);
  closeSync(openSync(destination, 'wx'));
  return destination;
}
