/**
 * Where the CLI remembers your key. One file, mode 600, this is a credential,
 * and it is the identity for everything the control plane does on your behalf.
 */
import {
  chmodSync,
  closeSync,
  constants as fsConstants,
  mkdirSync,
  openSync,
  readFileSync,
  renameSync,
  unlinkSync,
  writeFileSync,
} from 'node:fs';
import { randomUUID } from 'node:crypto';
import { homedir } from 'node:os';
import { basename, join, dirname } from 'node:path';
import { CONFIG_DIR_MODE, CONFIG_FILE_MODE, DEFAULT_BASE_URL, JSON_INDENT } from './constants.ts';

/** The saved config file: OYA_CONFIG_HOME, or ~/.oya. */
const FILE = join(process.env.OYA_CONFIG_HOME || join(homedir(), '.oya'), 'config.json');

/** What `oya login` saves. */
export interface CliConfig {
  /** The API key. */
  apiKey?: string;
  /** The control plane it belongs to. */
  baseUrl?: string;
}

/** The saved config, or nothing when there is none or it does not parse. */
export function load(): CliConfig {
  try {
    return JSON.parse(readFileSync(FILE, 'utf8')) as CliConfig;
  } catch {
    return {};
  }
}

/** Ensures the config folder exists and is owner-only where supported. */
function ensureConfigDir(dir: string): void {
  mkdirSync(dir, { recursive: true, mode: CONFIG_DIR_MODE });
  try {
    chmodSync(dir, CONFIG_DIR_MODE);
  } catch {
    /* Windows or read-only parent */
  }
}

/** Opens a new owner-only temporary file exclusively, refusing symlinks. */
function openTempFile(tmp: string, mode: number): number {
  const { O_WRONLY, O_CREAT, O_EXCL, O_NOFOLLOW = 0 } = fsConstants;
  return openSync(tmp, O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW, mode);
}

/** Writes `text` to `fd` and closes it even if writing fails. */
function writeFd(fd: number, text: string): void {
  try {
    writeFileSync(fd, text);
  } finally {
    closeSync(fd);
  }
}

/** Removes a leftover temporary file, ignoring missing-file errors. */
function removeTemp(tmp: string): void {
  try {
    unlinkSync(tmp);
  } catch {
    /* already removed */
  }
}

/** Atomically replaces `filePath` with `text` in a fresh owner-only file, never following a symlink. */
export function writePrivateFile(filePath: string, text: string, mode = CONFIG_FILE_MODE): void {
  const tmp = join(dirname(filePath), `.${basename(filePath)}.${process.pid}-${randomUUID()}.tmp`);
  try {
    writeFd(openTempFile(tmp, mode), text);
    renameSync(tmp, filePath);
  } catch (err) {
    removeTemp(tmp);
    throw err;
  }
}

/** Merges `values` into the saved config atomically, owner-only, never following a symlink. */
export function save(values: CliConfig): void {
  const text = JSON.stringify({ ...load(), ...values }, null, JSON_INDENT);
  ensureConfigDir(dirname(FILE));
  writePrivateFile(FILE, text, CONFIG_FILE_MODE);
}

/** Where the config lives, for messages. */
export const configPath = FILE;

/** Where a key came from, as messages name it. */
export type KeySource = '--key' | 'OYA_API_KEY' | 'saved' | 'none';
/** Where an address came from, as messages name it. */
export type UrlSource = '--url' | 'OYA_BASE_URL' | 'saved' | 'default';

/** The key and address a call uses, and where each came from, so a failure can name the one to fix. */
export interface Origin {
  /** The API key, or '' when there is none. */
  apiKey: string;
  /** Where it came from. */
  apiKeyFrom: KeySource;
  /** The control plane's address, without a trailing slash. */
  baseUrl: string;
  /** Where it came from. */
  baseUrlFrom: UrlSource;
  /** Whether `oya login` also saved a key, which a flag or the environment overrides. */
  savedKey: boolean;
  /** The control plane that saved key is for, when the file says. */
  savedUrl?: string;
}

/** The first of flag, environment and saved file that has a value, with its source. */
function first<S extends string>(
  flag: string | undefined,
  env: string | undefined,
  saved: string | undefined,
  names: [S, S, S],
) {
  if (flag) return { value: flag, from: names[0] };
  if (env) return { value: env, from: names[1] };
  return saved ? { value: saved, from: names[2] } : null;
}

/** Flags beat the environment, which beats the saved file, so CI never needs `oya login`. */
export function origin({ apiKey, baseUrl }: CliConfig = {}): Origin {
  const saved = load();
  const url = first(baseUrl, process.env.OYA_BASE_URL, saved.baseUrl, URL_SOURCES);
  const address = (url?.value ?? DEFAULT_BASE_URL).replace(/\/+$/, '');
  const key = first(apiKey, process.env.OYA_API_KEY, savedKeyFor(saved, address), KEY_SOURCES);
  const found = { apiKey: key?.value ?? '', apiKeyFrom: key?.from ?? ('none' as const), baseUrl: address };
  return { ...found, baseUrlFrom: url?.from ?? 'default', savedKey: !!saved.apiKey, savedUrl: saved.baseUrl };
}

/**
 * The saved key, only when the call goes to the control plane `oya login` saved
 * it for. `--url` or OYA_BASE_URL pointing elsewhere must not carry this key to
 * another host, which at best answers "Invalid API key" and at worst keeps it.
 * A file with no address, which `oya login` never writes, keeps the old behaviour.
 */
function savedKeyFor(saved: CliConfig, address: string): string | undefined {
  if (!saved.baseUrl || deploymentOf(saved.baseUrl) === deploymentOf(address)) return saved.apiKey;
  return undefined;
}

/** A control plane's address in one spelling: scheme and host lowercased, default port and trailing slashes dropped. */
function deploymentOf(url: string): string {
  try {
    const { origin: site, pathname } = new URL(String(url).trim());
    return `${site}${pathname.replace(/\/+$/, '')}`;
  } catch {
    return String(url).trim().replace(/\/+$/, '');
  }
}

/** A key's sources, in the order they win. */
const KEY_SOURCES: ['--key', 'OYA_API_KEY', 'saved'] = ['--key', 'OYA_API_KEY', 'saved'];
/** An address's sources, in the order they win. */
const URL_SOURCES: ['--url', 'OYA_BASE_URL', 'saved'] = ['--url', 'OYA_BASE_URL', 'saved'];

/** The key and address from the environment, else the saved file, else the default. */
export function resolved(): Required<CliConfig> {
  const { apiKey, baseUrl } = origin();
  return { apiKey, baseUrl };
}
