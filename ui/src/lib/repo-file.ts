/** Finds a file that lives at the repository root, wherever the site was started from. */
import { existsSync } from 'node:fs';
import path from 'node:path';

/**
 * The path of a repository-root file. The site runs from `ui/` in the image and under `cd ui && npm run dev`, but
 * from the repository root under the root `npm run dev`; assuming one of the two made /trust and /release-notes
 * fail with a missing file under the other.
 */
export function repoFile(...parts: string[]) {
  const fromRoot = path.join(process.cwd(), ...parts);
  return existsSync(fromRoot) ? fromRoot : path.join(process.cwd(), '..', ...parts);
}
