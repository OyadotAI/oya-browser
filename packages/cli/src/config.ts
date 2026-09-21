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

/** Flags and environment beat the saved file, so CI never needs `oya login`. */
export function resolved(): Required<CliConfig> {
  const saved = load();
  return {
    apiKey: process.env.OYA_API_KEY || saved.apiKey || '',
    baseUrl: (process.env.OYA_BASE_URL || saved.baseUrl || DEFAULT_BASE_URL).replace(/\/+$/, ''),
  };
}
