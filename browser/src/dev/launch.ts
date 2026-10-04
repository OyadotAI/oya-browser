/**
 * Starts the desktop browser in development (`npm start`, `npm run dev`). Node
 * runs this file directly (`node src/dev/launch.ts`, types stripped); it is not
 * part of the app's build.
 *
 * macOS uses the executable bundle's identity for the application menu, even
 * when app.setName() and the menu template have the correct product name.
 * Keep the installed Electron runtime untouched and cache a branded dev copy.
 * `--prepare` only prints the executable's path. The Electron tests import
 * developmentExecutable() to launch the same copy.
 */
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { spawn, execFileSync } from 'node:child_process';

/** process.argv index of the first argument after `node launch.ts`. */
const USER_ARGS_START = 2;

/** The browser package's folder: what Electron runs, and where build/ and node_modules/ are. */
const BROWSER_DIR = path.resolve(import.meta.dirname, '..', '..');

/** Resolves the electron package from the browser folder. */
const require = createRequire(import.meta.url);

/** The part of electron/package.json the cache is keyed on. */
interface ElectronPackage {
  /** The installed Electron's version. */
  version: string;
}

/** Info.plist keys that brand the copied bundle. */
const BRANDING = {
  CFBundleName: 'Oya Browser',
  CFBundleDisplayName: 'Oya Browser',
  CFBundleIdentifier: 'ai.oya.browser.development',
};

/** Copies Electron's app bundle to `staging` and brands it: name, id, icon, ad-hoc signature. */
function brandBundle(source: string, staging: string): void {
  fs.cpSync(source, staging, { recursive: true, verbatimSymlinks: true });
  const plist = path.join(staging, 'Contents', 'Info.plist');
  for (const [key, value] of Object.entries(BRANDING)) {
    execFileSync('/usr/libexec/PlistBuddy', ['-c', `Set :${key} ${value}`, plist]);
  }
  const icon = path.join(staging, 'Contents', 'Resources', 'electron.icns');
  fs.copyFileSync(path.join(BROWSER_DIR, 'build', 'icon.icns'), icon);
  execFileSync('/usr/bin/codesign', ['--force', '--deep', '--sign', '-', staging], { stdio: 'pipe' });
}

/** Moves the staged copy into place (unless another launch got there first) and marks it ready. */
function publishBundle(staging: string, bundle: string, ready: string): void {
  if (!fs.existsSync(bundle)) fs.renameSync(staging, bundle);
  fs.writeFileSync(ready, 'Oya Browser\n');
}

/** Builds the branded bundle in `cache` once, through a per-process staging copy. */
function buildBrandedBundle(source: string, cache: string, bundle: string, ready: string): void {
  fs.mkdirSync(cache, { recursive: true });
  const staging = path.join(cache, `staging-${process.pid}.app`);
  try {
    brandBundle(source, staging);
    publishBundle(staging, bundle, ready);
  } finally {
    if (fs.existsSync(staging)) fs.rmSync(staging, { recursive: true, force: true });
  }
}

/** The Electron executable to run: the stock one, or on macOS the cached branded copy. */
export function developmentExecutable(): string {
  // In Node (not Electron), the electron package exports its executable's path.
  const electron = require('electron') as string;
  if (process.platform !== 'darwin') return electron;
  const { version } = require('electron/package.json') as ElectronPackage;
  const cache = path.join(BROWSER_DIR, 'node_modules', '.cache', 'oya-runtime', `${version}-v1`);
  const bundle = path.join(cache, 'Oya Browser.app');
  const ready = path.join(cache, 'ready');
  if (!fs.existsSync(ready)) buildBrandedBundle(path.resolve(electron, '../../..'), cache, bundle, ready);
  return path.join(bundle, 'Contents', 'MacOS', 'Electron');
}

/** Runs the browser as a child, passing signals through and exiting with its code. */
function runBrowser(executable: string): void {
  const child = spawn(executable, [BROWSER_DIR, ...process.argv.slice(USER_ARGS_START)], { stdio: 'inherit' });
  child.on('error', (error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
  child.on('exit', (code, signal) => (signal ? process.kill(process.pid, signal) : (process.exitCode = code || 0)));
  for (const signal of ['SIGINT', 'SIGTERM'] as const) process.on(signal, () => child.kill(signal));
}

/** Prints the executable (`--prepare`) or runs the browser with it. */
function main(): void {
  const executable = developmentExecutable();
  if (process.argv.includes('--prepare')) process.stdout.write(executable + '\n');
  else runBrowser(executable);
}

// Run only as `node src/dev/launch.ts`, not when a test imports developmentExecutable().
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
