/** Resolve native agent commands per OS without sending credentials through a command shell. */
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFile, type ExecFileOptions } from 'node:child_process';
import { promisify } from 'node:util';
import { AGENT_CLI_TIMEOUT_MS } from './constants.ts';

/** Inputs allow Windows layouts and argument safety to be tested on any OS. */
export interface AgentPlatform {
  /** Runtime operating system. */
  platform: string;
  /** Actual user home, never modified by discovery. */
  home: string;
  /** OS environment used for native PATH and application data. */
  env: NodeJS.ProcessEnv;
  /** Existence check is replaced by scratch fixtures in tests. */
  exists(file: string): boolean;
  /** Process seam keeps tests isolated while preserving execFile argument arrays. */
  run?: (file: string, args: string[], options: ExecFileOptions) => Promise<unknown>;
}
/** An executable plus fixed prefix arguments, never a shell command string. */
export interface AgentCommand {
  /** Native executable to pass directly to execFile. */
  file: string;
  /** Known JavaScript entrypoint, when a supported npm shim needs Node. */
  args: string[];
}
/** Capture platform inputs at the call boundary so per-user tests stay isolated. */
export const agentPlatform = (): AgentPlatform => ({
  platform: process.platform,
  home: os.homedir(),
  env: process.env,
  exists: fs.existsSync,
});
/** Windows environment names are case-insensitive, including Path and APPDATA. */
function envValue(deps: AgentPlatform, key: string): string {
  return Object.entries(deps.env).find(([name]) => name.toLowerCase() === key.toLowerCase())?.[1] || '';
}
/** Native path operations and separators follow the target platform even in host-independent tests. */
const paths = (deps: AgentPlatform) => (deps.platform === 'win32' ? path.win32 : path.posix);
/** Supported package entrypoints replace npm's command-shell wrappers. */
const NPM_ENTRIES: Record<string, string[]> = {
  claude: ['@anthropic-ai', 'claude-code', 'cli.js'],
  codex: ['@openai', 'codex', 'bin', 'codex.js'],
};
/** Per-platform install roots keep discovery readable and native. */
function extraBinDirs(deps: AgentPlatform): string[] {
  const p = paths(deps);
  return deps.platform === 'win32'
    ? [
        p.join(envValue(deps, 'APPDATA') || p.join(deps.home, 'AppData', 'Roaming'), 'npm'),
        p.join(envValue(deps, 'ProgramFiles') || 'C:\\Program Files', 'nodejs'),
      ]
    : [p.join(deps.home, '.npm-global', 'bin'), '/opt/homebrew/bin', '/usr/local/bin'];
}
/** Known per-user installs supplement PATH when an app is opened from the desktop. */
export function agentBinDirs(deps = agentPlatform()): string[] {
  const p = paths(deps);
  const usual = [p.join(deps.home, '.local', 'bin'), p.join(deps.home, '.claude', 'local')];
  const extra = extraBinDirs(deps);
  return [...new Set([...usual, ...extra, ...envValue(deps, 'PATH').split(p.delimiter).filter(Boolean)])];
}
/** Resolve supported npm entrypoints directly; arbitrary .cmd files are never executed or parsed. */
function npmCommand(name: string, dir: string, deps: AgentPlatform): AgentCommand | null {
  if (!Object.hasOwn(NPM_ENTRIES, name)) return null;
  const entry = path.win32.join(dir, 'node_modules', ...NPM_ENTRIES[name]);
  const node = agentBinDirs(deps)
    .map((folder) => path.win32.join(folder, 'node.exe'))
    .find(deps.exists);
  return node && deps.exists(entry) ? { file: node, args: [entry] } : null;
}
/** Prefer native executables, then known npm packages, with no command-shell fallback. */
export function agentCommand(name: string, deps = agentPlatform()): AgentCommand | null {
  const p = paths(deps);
  const dirs = agentBinDirs(deps);
  const native = dirs.map((dir) => p.join(dir, deps.platform === 'win32' ? `${name}.exe` : name)).find(deps.exists);
  if (native) return { file: native, args: [] };
  if (deps.platform !== 'win32') return null;
  return findNpmCommand(name, deps, dirs);
}
/** Only known installed npm packages can provide a non-native command. */
function findNpmCommand(name: string, deps: AgentPlatform, dirs: string[]): AgentCommand | null {
  return (
    dirs
      .filter((dir) => deps.exists(path.win32.join(dir, `${name}.cmd`)))
      .map((dir) => npmCommand(name, dir, deps))
      .find(Boolean) || null
  );
}
/** Existing settings paths follow the editor's own OS convention. */
export function vscodeConfig(deps = agentPlatform()): string {
  const p = paths(deps);
  if (deps.platform === 'win32')
    return p.join(envValue(deps, 'APPDATA') || p.join(deps.home, 'AppData', 'Roaming'), 'Code', 'User', 'mcp.json');
  return deps.platform === 'darwin'
    ? p.join(deps.home, 'Library', 'Application Support', 'Code', 'User', 'mcp.json')
    : p.join(envValue(deps, 'XDG_CONFIG_HOME') || p.join(deps.home, '.config'), 'Code', 'User', 'mcp.json');
}

/** Resolve editor applications from fixed native installation locations, never renderer paths. */
export function editorApplication(id: string, deps = agentPlatform()): string | null {
  const names: Record<string, string> = { cursor: 'Cursor', vscode: 'Visual Studio Code' };
  if (!Object.hasOwn(names, id)) return null;
  const name = names[id];
  const candidates = editorPaths(name, id, deps);
  return candidates.find(deps.exists) || null;
}
/** macOS bundles and Windows native programs are the supported editor installations. */
function editorPaths(name: string, id: string, deps: AgentPlatform): string[] {
  return deps.platform === 'darwin'
    ? [`/Applications/${name}.app`, paths(deps).join(deps.home, 'Applications', `${name}.app`)]
    : windowsEditors(id, deps);
}
/** Windows editors have stable per-user and machine install directories. */
function windowsEditors(id: string, deps: AgentPlatform): string[] {
  if (deps.platform !== 'win32') return [];
  const p = paths(deps),
    local = envValue(deps, 'LOCALAPPDATA') || p.join(deps.home, 'AppData', 'Local');
  const program = envValue(deps, 'ProgramFiles') || 'C:\\Program Files';
  const folder = id === 'cursor' ? 'cursor' : 'Microsoft VS Code';
  const executable = id === 'cursor' ? 'Cursor.exe' : 'Code.exe';
  return [p.join(local, 'Programs', folder, executable), p.join(program, folder, executable)];
}

/** An installation needs a real command or editor application, not a leftover settings directory. */
export function agentInstallation(id: string, deps = agentPlatform()): AgentCommand | null {
  const cli: Record<string, string> = { 'claude-code': 'claude', codex: 'codex' };
  if (Object.hasOwn(cli, id)) return agentCommand(cli[id], deps);
  const file = editorApplication(id, deps);
  return file ? { file, args: [] } : null;
}
/** Bound setup work and keep every token/URL as a distinct process argument. */
export async function runAgent(name: string, args: string[], deps = agentPlatform()): Promise<void> {
  const command = agentCommand(name, deps);
  if (!command) throw Error(`${name} is not installed with a supported executable. Install it, then check again.`);
  try {
    await executeAgent(command, args, deps);
  } catch {
    throw Error(`Could not configure ${name}. Check its installation and permissions, then try again.`);
  }
}
/** A single bounded execFile preserves punctuation in paths, URLs and credentials as data. */
async function executeAgent(command: AgentCommand, args: string[], deps: AgentPlatform): Promise<void> {
  const env = Object.fromEntries(Object.entries(deps.env).filter(([key]) => key.toLowerCase() !== 'path'));
  const options = {
    env: { ...env, PATH: agentBinDirs(deps).join(paths(deps).delimiter) },
    timeout: AGENT_CLI_TIMEOUT_MS,
    windowsHide: true,
  };
  await (deps.run || promisify(execFile))(command.file, [...command.args, ...args], options);
}
