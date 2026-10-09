/** Prepare a separate, launchable macOS development bundle without changing the native QA engine. */
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { nativeDevelopmentEntry } from '../src/dev/native-entry.ts';

/** This script is local tooling, not the production packaging or signing pipeline. */
const root = path.resolve(import.meta.dirname, '..');
/** An explicit source avoids silently substituting the stock runtime for the patched engine. */
const [, , source] = process.argv;
/** A stable, unique identity keeps older diagnostic copies from receiving this app's links. */
const bundleId = 'ai.oya.browser.native-development';
/** Manual testing must not replace the user's installed browser profile. */
const profile = path.join(root, 'node_modules/.cache/oya-native-manual-profile');
/** This copy owns its entry point; QA launchers continue to use the source bundle. */
const bundle = path.join(root, 'node_modules/.cache/oya-native-manual/Oya Browser.app');

/** Refuse non-macOS, missing engines and overwriting an existing (possibly running) manual bundle. */
function validateSource() {
  if (process.platform !== 'darwin' || !source) throw Error('Pass the patched Oya .app on macOS');
  if (!fs.existsSync(path.join(source, 'Contents/MacOS/Electron'))) throw Error('Missing Oya engine executable');
  if (!fs.existsSync(path.join(root, 'out/main/index.js'))) throw Error('Run npm run build first');
  if (fs.existsSync(bundle))
    throw Error('Manual bundle already exists; close and move it before preparing a replacement');
}

/** The dev entry is loaded even when LaunchServices supplies no app-path argument. */
function writeEntry() {
  const target = path.join(bundle, 'Contents/Resources/app');
  fs.mkdirSync(target, { recursive: true });
  const { version } = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8'));
  fs.writeFileSync(
    path.join(target, 'package.json'),
    JSON.stringify({ name: 'oya-browser', version, main: 'bootstrap.cjs' }),
  );
  fs.writeFileSync(path.join(target, 'bootstrap.cjs'), nativeDevelopmentEntry(root, profile));
}

/** Declare the scheme in Info.plist: macOS ignores Electron's Windows-only interpreter arguments. */
function declareProtocol() {
  const plist = path.join(bundle, 'Contents/Info.plist');
  execFileSync('/usr/bin/plutil', ['-replace', 'CFBundleIdentifier', '-string', bundleId, plist]);
  const protocols = [{ CFBundleURLName: bundleId, CFBundleURLSchemes: ['oya'] }];
  execFileSync('/usr/bin/plutil', ['-replace', 'CFBundleURLTypes', '-json', JSON.stringify(protocols), plist]);
}

/** APFS cloning keeps the patched engine exact without duplicating its disk blocks. */
function prepare() {
  validateSource();
  fs.mkdirSync(path.dirname(bundle), { recursive: true });
  execFileSync('/bin/cp', ['-cR', path.resolve(source), bundle]);
  writeEntry();
  declareProtocol();
  execFileSync('/usr/bin/codesign', ['--force', '--deep', '--sign', '-', bundle], { stdio: 'inherit' });
  console.log(bundle);
}
prepare();
