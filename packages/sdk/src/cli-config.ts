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
interface SavedConfig {
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
};

/** The part of `node:fs` this file reads through. */
type Fs = {
  /** Whether the config file is there at all. */
  existsSync(path: string): boolean;
  /** Its contents. */
  readFileSync(path: string, encoding: string): string;
  /** Makes the config folder. */
  mkdirSync(path: string, options: { /** Parents too. */ recursive: boolean }): void;
  /** Writes the file. */
  writeFileSync(path: string, data: string, options: { /** Owner only: it holds a key. */ mode: number }): void;
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

/** Owner read and write only: the file holds a key. */
const PRIVATE_FILE = 0o600;
/** Indent of the saved JSON, as `oya login` writes it. */
const JSON_INDENT = 2;

/** Saves a key where `oya login` would, unless one is there already. True when it wrote. */
export function saveKey(apiKey: string, baseUrl: string): boolean {
  const fs = builtin<Fs>('node:fs');
  const file = configFile();
  if (!fs || !file || savedConfig().apiKey) return false;
  fs.mkdirSync(file.slice(0, file.lastIndexOf('/')), { recursive: true });
  fs.writeFileSync(file, JSON.stringify({ ...savedConfig(), apiKey, baseUrl }, null, JSON_INDENT), {
    mode: PRIVATE_FILE,
  });
  return true;
}
