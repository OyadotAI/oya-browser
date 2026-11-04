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
 * Interactive macOS launches go through Launch Services for native privacy
 * attribution. It owns the GUI lifetime; interrupting this waiter leaves Oya open.
 */
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { spawn, execFileSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { USER_ARGS_START, DEVELOPMENT_LOG_MODE, LAUNCH_FAILURE } from './constants.ts';

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

/** Private files retain startup failures when Launch Services cannot inherit terminal output. */
function developmentLogs(): string[] {
  const directory = fs.mkdtempSync(path.join(tmpdir(), 'oya-development-'));
  const logs = ['stdout.log', 'stderr.log'].map((name) => path.join(directory, name));
  for (const log of logs) fs.writeFileSync(log, '', { mode: DEVELOPMENT_LOG_MODE });
  console.error(`Oya development logs: ${directory}`);
  console.error('Close Oya to quit the app. Ctrl+C stops only this launch waiter.');
  return logs;
}

/** Native launch requests preserve argument boundaries without a shell. */
interface DevelopmentCommand {
  /** Exact executable or system Launch Services entry. */ command: string;
  /** Each argument remains separate, including paths containing spaces. */ args: string[];
}
/** Use the exact branded app through macOS while fixture callers retain direct executable control. */
export function developmentCommand(executable: string, args: string[], logs: string[]): DevelopmentCommand {
  if (process.platform !== 'darwin') return { command: executable, args };
  const start = ['-n', '-W', '-a', path.resolve(executable, '../../..')];
  const output = ['--stdout', logs[0], '--stderr', logs[1]];
  return { command: '/usr/bin/open', args: [...start, ...output, '--args', ...args] };
}

/** A macOS waiter reports its own result; other platforms preserve direct-child signal behavior. */
function finished(code: number | null, signal: NodeJS.Signals | null): void {
  if (signal && process.platform !== 'darwin') process.kill(process.pid, signal);
  else process.exitCode = code ?? LAUNCH_FAILURE;
}

/** Startup failure stays visible in the terminal even when no application was created. */
function failed(error: Error): void {
  console.error(error.message);
  process.exitCode = LAUNCH_FAILURE;
}

/** Signals target only this launcher's child, which is a waiter rather than Oya on macOS. */
function runBrowser(executable: string): void {
  const logs = process.platform === 'darwin' ? developmentLogs() : [];
  const request = developmentCommand(executable, [BROWSER_DIR, ...process.argv.slice(USER_ARGS_START)], logs);
  const child = spawn(request.command, request.args, { stdio: 'inherit', env: process.env, shell: false });
  child.on('error', failed);
  child.on('exit', finished);
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
