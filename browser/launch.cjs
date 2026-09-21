/**
 * Starts the desktop browser in development (`npm start`, `npm run dev`).
 *
 * macOS uses the executable bundle's identity for the application menu, even
 * when app.setName() and the menu template have the correct product name.
 * Keep the installed Electron runtime untouched and cache a branded dev copy.
 * `--prepare` only prints the executable's path (the shell tests use it).
 */
const fs = require('node:fs');
const path = require('node:path');
const { spawn, execFileSync } = require('node:child_process');
const { USER_ARGS_START } = require('./constants.cjs');

/** Info.plist keys that brand the copied bundle. */
const BRANDING = {
  CFBundleName: 'Oya Browser',
  CFBundleDisplayName: 'Oya Browser',
  CFBundleIdentifier: 'ai.oya.browser.development',
};

/** Copies Electron's app bundle to `staging` and brands it: name, id, icon, ad-hoc signature. */
function brandBundle(source, staging) {
  fs.cpSync(source, staging, { recursive: true, verbatimSymlinks: true });
  const plist = path.join(staging, 'Contents', 'Info.plist');
  for (const [key, value] of Object.entries(BRANDING)) {
    execFileSync('/usr/libexec/PlistBuddy', ['-c', `Set :${key} ${value}`, plist]);
  }
  const icon = path.join(staging, 'Contents', 'Resources', 'electron.icns');
  fs.copyFileSync(path.join(__dirname, 'build', 'icon.icns'), icon);
  execFileSync('/usr/bin/codesign', ['--force', '--deep', '--sign', '-', staging], { stdio: 'pipe' });
}

/** Moves the staged copy into place (unless another launch got there first) and marks it ready. */
function publishBundle(staging, bundle, ready) {
  if (!fs.existsSync(bundle)) fs.renameSync(staging, bundle);
  fs.writeFileSync(ready, 'Oya Browser\n');
}

/** Builds the branded bundle in `cache` once, through a per-process staging copy. */
function buildBrandedBundle(source, cache, bundle, ready) {
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
function developmentExecutable() {
  const electron = require('electron');
  if (process.platform !== 'darwin') return electron;
  const version = require('electron/package.json').version;
  const cache = path.join(__dirname, 'node_modules', '.cache', 'oya-runtime', `${version}-v1`);
  const bundle = path.join(cache, 'Oya Browser.app');
  const ready = path.join(cache, 'ready');
  if (!fs.existsSync(ready)) buildBrandedBundle(path.resolve(electron, '../../..'), cache, bundle, ready);
  return path.join(bundle, 'Contents', 'MacOS', 'Electron');
}

/** Runs the browser as a child, passing signals through and exiting with its code. */
function runBrowser(executable) {
  const child = spawn(executable, [__dirname, ...process.argv.slice(USER_ARGS_START)], { stdio: 'inherit' });
  child.on('error', (error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
  child.on('exit', (code, signal) => (signal ? process.kill(process.pid, signal) : (process.exitCode = code || 0)));
  for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
}

if (require.main === module) {
  const executable = developmentExecutable();
  if (process.argv.includes('--prepare')) process.stdout.write(executable + '\n');
  else runBrowser(executable);
}
module.exports = { developmentExecutable };
