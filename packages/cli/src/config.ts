/**
 * Where the CLI remembers your key. One file, mode 600, this is a credential,
 * and it is the identity for everything the control plane does on your behalf.
 */
import { readFileSync, writeFileSync, mkdirSync, chmodSync } from 'node:fs';
import { homedir } from 'node:os';
import { join, dirname } from 'node:path';
import { CONFIG_FILE_MODE, DEFAULT_BASE_URL, JSON_INDENT } from './constants.ts';

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

/** Merges `values` into the saved config, owner-only. */
export function save(values: CliConfig): void {
  const merged = { ...load(), ...values };
  mkdirSync(dirname(FILE), { recursive: true });
  writeFileSync(FILE, JSON.stringify(merged, null, JSON_INDENT), { mode: CONFIG_FILE_MODE });
  chmodSync(FILE, CONFIG_FILE_MODE); // an existing file keeps its old mode without this
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
  const key = first(apiKey, process.env.OYA_API_KEY, saved.apiKey, KEY_SOURCES);
  const url = first(baseUrl, process.env.OYA_BASE_URL, saved.baseUrl, URL_SOURCES);
  const address = (url?.value ?? DEFAULT_BASE_URL).replace(/\/+$/, '');
  const found = { apiKey: key?.value ?? '', apiKeyFrom: key?.from ?? ('none' as const), baseUrl: address };
  return { ...found, baseUrlFrom: url?.from ?? 'default', savedKey: !!saved.apiKey };
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
