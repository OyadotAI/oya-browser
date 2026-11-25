/**
 * The .env file the wizard writes: reading what a previous install decided,
 * rendering values into commented sections, and writing it owner-only.
 */
import { existsSync, readFileSync } from 'node:fs';
import { writePrivateFile } from '../config.ts';
import { ENV_FILE_MODE } from './constants.ts';

/**
 * Values that must never be printed. Named explicitly rather than matched on the
 * variable name: DATABASE_URL embeds a password and contains none of the words a
 * pattern would look for.
 */
export const SECRET_KEYS = new Set([
  'OYA_PROFILE_SECRET',
  'OYA_OPERATOR_TOKEN',
  'OYA_METRICS_TOKEN',
  'API_KEYS',
  'FLEET_TOKEN',
  'OYA_CLUSTER_SECRET',
  'DATABASE_URL',
  'SUPABASE_SERVICE_KEY',
  'OPENAI_API_KEY',
  'OYA_CLOUD_API_KEY',
  'ANCHOR_API_KEY',
  'BROWSERBASE_API_KEY',
  'STEEL_API_KEY',
  'BROWSERUSE_API_KEY',
  'OYA_CAPTCHA_API_KEY',
  'OYA_CDP_WS_URL',
]);

/** .env's sections: a comment title and the variables under it, in order. */
const SECTIONS: Array<[string, string[]]> = [
  ['Server', ['PORT', 'OYA_UI_MODE', 'OYA_DATA_DIR']],
  ['Tenant keys, each is an identity; add more from the dashboard', ['API_KEYS']],
  ['Host operations: /metrics, drain, fleet provision', ['OYA_OPERATOR_TOKEN', 'OYA_METRICS_TOKEN']],
  ['Credentials at rest. Lose this and every stored cookie, proxy and TOTP seed is unreadable', ['OYA_PROFILE_SECRET']],
  ['Database', ['OYA_STORAGE', 'DATABASE_URL', 'SUPABASE_URL', 'SUPABASE_SERVICE_KEY', 'OYA_RECORDING_BUCKET']],
  [
    'Agent LLM. A private or http base URL is operator-only: the dashboard and\n# POST /config/host reject one, on purpose, tenants can set that field too',
    ['OPENAI_API_KEY', 'OPENAI_BASE_URL', 'CHAT_MODEL'],
  ],
  ['Browser fleet', ['OYA_BROWSER_PROVIDER', 'OYA_FLEET_RUNTIME', 'OYA_PUBLIC_WS_URL', 'OYA_CDP_WS_URL']],
  [
    'Governed Docker runtime',
    [
      'OYA_MANAGED_NETWORK',
      'OYA_MANAGED_IMAGE',
      'OYA_MANAGED_CONTROL_URL',
      'OYA_MANAGED_PROXY_URL',
      'OYA_MANAGED_REGION',
      'OYA_EGRESS_PORT',
      'OYA_EGRESS_HOST',
    ],
  ],
  ['Oya Cloud', ['OYA_CLOUD_API_KEY', 'OYA_CLOUD_SNAPSHOT', 'OYA_CLOUD_TARGET']],
  [
    'Hosted CDP vendors',
    ['BROWSERBASE_API_KEY', 'BROWSERBASE_PROJECT_ID', 'STEEL_API_KEY', 'ANCHOR_API_KEY', 'BROWSERUSE_API_KEY'],
  ],
  ['CAPTCHA', ['OYA_CAPTCHA_PROVIDER', 'OYA_CAPTCHA_API_KEY']],
  ['Multi-replica. One SQLite control database allows exactly one writer', ['OYA_INSTANCE_URL', 'OYA_CLUSTER_SECRET']],
];

/** Every variable name owned by a managed .env section (plus legacy aliases). */
export const MANAGED_KEYS = new Set([...SECTIONS.flatMap(([, keys]) => keys), 'DAYTONA_SNAPSHOT']);

/** Unquotes a dotenv double-quoted value, restoring escaped quotes, backslashes and newlines. */
function unquoteDouble(raw: string): string {
  return raw.slice(1, -1).replace(/\\(["\\nr])/g, (_m, c) => (c === 'n' ? '\n' : c === 'r' ? '\r' : c));
}

/** Parses one dotenv value: single-quoted, double-quoted, or unquoted with trailing `#` comments stripped. */
function parseEnvValue(raw: string): string {
  const trimmed = raw.trim();
  if (trimmed.length > 1 && trimmed.startsWith("'") && trimmed.endsWith("'")) return trimmed.slice(1, -1);
  if (trimmed.length > 1 && trimmed.startsWith('"') && trimmed.endsWith('"')) return unquoteDouble(trimmed);
  return trimmed.replace(/\s+#.*$/, '').trim();
}

/** Quotes a dotenv value when it contains `#`, whitespace, quotes, or backslashes so `dotenv` reads it back intact. */
function formatEnvValue(value: string): string {
  if (!/[\s#"'\\]/.test(value)) return value;
  const escaped = value.replace(/\\/g, '\\\\').replace(/"/g, '\\"').replace(/\n/g, '\\n').replace(/\r/g, '\\r');
  return `"${escaped}"`;
}

/** Parse just enough to know what a previous install already decided. */
export function readEnv(path: string): Record<string, string> {
  if (!existsSync(path)) return {};
  const out: Record<string, string> = {};
  for (const line of readFileSync(path, 'utf8').split('\n')) {
    const m = /^(?:export\s+)?([A-Z0-9_]+)=(.*)$/.exec(line.trim());
    if (m) out[m[1]] = parseEnvValue(m[2]);
  }
  return out;
}

/** One titled block of `KEY=value` lines, followed by a blank line. */
function block(title: string, keys: string[], values: Record<string, string>): string[] {
  return [`# ── ${title} ──`, ...keys.map((k) => `${k}=${formatEnvValue(values[k])}`), ''];
}

/** The sectioned blocks for every value that has a section; records which were placed. */
function sectionLines(values: Record<string, string>, placed: Set<string>): string[] {
  const lines: string[] = [];
  for (const [title, keys] of SECTIONS) {
    const present = keys.filter((k) => values[k] !== undefined && values[k] !== '');
    present.forEach((k) => placed.add(k));
    if (present.length) lines.push(...block(title, present, values));
  }
  return lines;
}

/** Every value in its section, then any not in a section under "Other", then `extra`. */
export function renderEnv(values: Record<string, string>, extra: string[]): string {
  const lines = ['# Written by `oya install`. Re-run it, or edit by hand, both are fine.', ''];
  const placed = new Set<string>();
  lines.push(...sectionLines(values, placed));
  const leftover = Object.keys(values).filter((k) => !placed.has(k) && values[k]);
  if (leftover.length) lines.push(...block('Other', leftover, values));
  if (extra.length) lines.push(...extra, '');
  return lines.join('\n');
}

/** 0600 atomic write without following symlinks: this file holds the KEK and every provider credential. */
export function writeEnv(path: string, body: string): void {
  writePrivateFile(path, body, ENV_FILE_MODE);
}
