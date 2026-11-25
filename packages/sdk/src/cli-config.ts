/**
 * The credentials `oya login` saves. Without this, running the CLI's own
 * sign-in and then writing a script would still fail for want of a key, which
 * is a poor way to learn that the two read different places.
 *
 * Node only, and only through `process.getBuiltinModule` (Node 22.3+), so a
 * bundle for the browser carries no `node:` import and an older Node simply
 * falls back to the environment.
 */

/** What the CLI writes to its config file. */
export interface SavedConfig {
  /** The key it saved. */
  apiKey?: string;
  /** The deployment it saved it for. */
  baseUrl?: string;
}

/** Node's own process object, as much of it as this file needs. */
type NodeProcess = {
  /** Present from Node 22.3, the only synchronous way to a builtin without an import. */
  getBuiltinModule?: (name: string) => unknown;
  /** The environment, when there is one. */
  env?: Record<string, string | undefined>;
  /** Process id, for unique sibling temp files. */
  pid?: number;
};

/** The part of `node:fs` this file reads and writes through. */
type Fs = {
  /** Open flags for exclusive non-symlink creation. */
  constants: {
    /** Write-only open flag. */
    O_WRONLY: number;
    /** Create-if-missing flag. */
    O_CREAT: number;
    /** Fail-if-exists flag. */
    O_EXCL: number;
    /** Refuse-symlink flag where supported. */
    O_NOFOLLOW?: number;
  };
  /** Whether the config file is there at all. */
  existsSync(path: string): boolean;
  /** Its contents. */
  readFileSync(path: string, encoding: string): string;
  /** Makes the config folder. */
  mkdirSync(path: string, options: { /** Parents too. */ recursive: boolean; /** Folder mode. */ mode: number }): void;
  /** Tightens an existing folder's permissions. */
  chmodSync(path: string, mode: number): void;
  /** Opens a file descriptor with explicit flags and mode. */
  openSync(path: string, flags: number, mode: number): number;
  /** Writes text to an open descriptor. */
  writeFileSync(fd: number, data: string): void;
  /** Closes an open descriptor. */
  closeSync(fd: number): void;
  /** Atomically replaces `dest` with `src`. */
  renameSync(src: string, dest: string): void;
  /** Removes a leftover temp file on failure. */
  unlinkSync(path: string): void;
};

/** The part of `node:os` this file reads through. */
type Os = {
  /** Where `~` points. */
  homedir(): string;
};

/** The Node process, or undefined in a runtime that has none. */
const node = (): NodeProcess | undefined =>
  (globalThis as { /** Node's process, when there is one. */ process?: NodeProcess }).process;

/** A Node builtin, when this runtime has them and exposes them synchronously. */
export function builtin<T>(name: string): T | null {
  const get = node()?.getBuiltinModule;
  return typeof get === 'function' ? ((get.call(node(), name) as T) ?? null) : null;
}

/** The CLI's config file path: OYA_CONFIG_HOME, else ~/.oya/config.json. */
function configFile(): string | null {
  const os = builtin<Os>('node:os');
  const home = node()?.env?.OYA_CONFIG_HOME || (os ? `${os.homedir()}/.oya` : null);
  return home ? `${home}/config.json` : null;
}

/** What `oya login` saved, or an empty record when there is nothing to read. */
export function savedConfig(): SavedConfig {
  try {
    const fs = builtin<Fs>('node:fs');
    const file = configFile();
    if (!fs || !file || !fs.existsSync(file)) return {};
    return JSON.parse(fs.readFileSync(file, 'utf8')) as SavedConfig;
  } catch {
    return {}; // an unreadable or half-written file is not a reason to fail a call
  }
}

/** Owner access only on the config folder. */
const PRIVATE_DIR = 0o700;
/** Owner read and write only: the file holds a key. */
const PRIVATE_FILE = 0o600;
/** Indent of the saved JSON, as `oya login` writes it. */
const JSON_INDENT = 2;

/** Ensures the config folder exists and is owner-only where supported. */
function ensurePrivateDir(fs: Fs, dir: string): void {
  fs.mkdirSync(dir, { recursive: true, mode: PRIVATE_DIR });
  try {
    fs.chmodSync(dir, PRIVATE_DIR);
  } catch {
    /* Windows or read-only parent */
  }
}

/** Opens a new owner-only temporary file exclusively, refusing symlinks. */
function openTemp(fs: Fs, tmp: string): number {
  const { O_WRONLY, O_CREAT, O_EXCL, O_NOFOLLOW = 0 } = fs.constants;
  return fs.openSync(tmp, O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW, PRIVATE_FILE);
}

/** Writes `text` to `fd` and closes it even if writing fails. */
function writeFd(fs: Fs, fd: number, text: string): void {
  try {
    fs.writeFileSync(fd, text);
  } finally {
    fs.closeSync(fd);
  }
}

/** Removes a leftover temporary file, ignoring missing-file errors. */
function removeTemp(fs: Fs, tmp: string): void {
  try {
    fs.unlinkSync(tmp);
  } catch {
    /* already removed */
  }
}

/** Sibling temporary path next to `file`, unique to this call. */
const tempFile = (file: string): string =>
  `${file.slice(0, file.lastIndexOf('/'))}/.config.json.${node()?.pid ?? 0}-${globalThis.crypto.randomUUID()}.tmp`;

/** Atomically replaces `file` with a fresh owner-only regular file, never following a symlink. */
function writePrivate(fs: Fs, file: string, text: string): void {
  const tmp = tempFile(file);
  try {
    writeFd(fs, openTemp(fs, tmp), text);
    fs.renameSync(tmp, file);
  } catch (err) {
    removeTemp(fs, tmp);
    throw err;
  }
}

/** Saves a key where `oya login` would, unless one is there already. True when it wrote. */
export function saveKey(apiKey: string, baseUrl: string): boolean {
  const fs = builtin<Fs>('node:fs');
  const file = configFile();
  if (!fs || !file || savedConfig().apiKey) return false;
  ensurePrivateDir(fs, file.slice(0, file.lastIndexOf('/')));
  writePrivate(fs, file, JSON.stringify({ ...savedConfig(), apiKey, baseUrl }, null, JSON_INDENT));
  return true;
}
