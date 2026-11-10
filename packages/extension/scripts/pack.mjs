/**
 * Builds the two store uploads: dist/oya-chrome.zip and dist/oya-firefox.zip.
 * Both carry the same source files unchanged (no build step, so the Firefox
 * reviewers read exactly what runs); only the manifest differs, by the
 * Firefox-only settings block. The version comes from package.json.
 */
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

/** The package folder. */
const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');

/** Firefox's add-on id, which must never change once published. */
export const GECKO_ID = 'extension@oyabrowser.com';

/** The first Firefox with MV3 optional host permissions and data-collection declarations. */
export const GECKO_MIN_VERSION = '140.0';

/** The first Firefox for Android that reads the data-collection declarations. */
export const GECKO_ANDROID_MIN_VERSION = '142.0';

/** Files from src/ that ship. The base manifest is replaced by the built one. */
const SKIP = new Set(['manifest.base.json']);

/** The JSON in a file. */
const readJson = (path) => JSON.parse(readFileSync(path, 'utf8'));

/**
 * The manifest for one store. Firefox declares that the extension sends
 * authentication data (the person's cookies) to Oya and nothing else.
 */
export function manifestFor(target, base, version) {
  const manifest = { ...base, version };
  if (target !== 'firefox') return manifest;
  const gecko = { id: GECKO_ID, strict_min_version: GECKO_MIN_VERSION };
  const consent = { data_collection_permissions: { required: ['authenticationInfo'] } };
  const android = { strict_min_version: GECKO_ANDROID_MIN_VERSION };
  return { ...manifest, browser_specific_settings: { gecko: { ...gecko, ...consent }, gecko_android: android } };
}

/** Writes dist/<target>/ and zips it into dist/oya-<target>.zip. */
function pack(target, base, version) {
  const out = join(ROOT, 'dist', target);
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  cpSync(join(ROOT, 'src'), out, { recursive: true, filter: (src) => !SKIP.has(src.split('/').pop()) });
  writeFileSync(join(out, 'manifest.json'), `${JSON.stringify(manifestFor(target, base, version), null, 2)}\n`);
  rmSync(join(ROOT, 'dist', `oya-${target}.zip`), { force: true });
  execFileSync('zip', ['-qr', join('..', `oya-${target}.zip`), '.'], { cwd: out });
}

/** Builds both zips from the package's version and the base manifest. */
function main() {
  const { version } = readJson(join(ROOT, 'package.json'));
  const base = readJson(join(ROOT, 'src', 'manifest.base.json'));
  for (const target of ['chrome', 'firefox']) pack(target, base, version);
  console.log(`Packed ${version}: dist/oya-chrome.zip, dist/oya-firefox.zip`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main();
