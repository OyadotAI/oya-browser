/**
 * Command-line parsing: the command, its positional arguments, and `--flags`.
 * Every flag the CLI knows is named here, once: a switch is always `true` and
 * leaves the next word alone; a value flag takes the next word, or its
 * `--flag=value` form. A flag the CLI does not know is a usage error rather
 * than silently ignored, since an ignored `--ws_url` looked like it worked.
 */
import { readFileSync } from 'node:fs';
import { invalidJson, usage } from './errors.ts';

/** Parsed flags: a value, or `true` for a switch. */
export type Flags = Record<string, string | boolean>;

/** What `oya` was asked to do. */
export interface Invocation {
  /** The command; `help` when none was given. */
  command: string;
  /** Positional arguments after it. */
  args: string[];
  /** Its flags. */
  flags: Flags;
}

/**
 * Flags that are switches. Without this list `oya stop --force <id>` read the id
 * as the value of --force and then asked for the id it had just swallowed.
 */
export const SWITCHES = new Set([
  'all',
  'debug',
  'dry-run',
  'force',
  'governed',
  'help',
  'json',
  'live',
  'preview',
  'replace',
  'version',
]);

/** Flags that take a value, each with an example for the message when the value is missing. */
export const VALUE_FLAGS: Record<string, string> = {
  after: '--after 120',
  'budget-usd': '--budget-usd 5',
  config: '--config oya-install.json',
  format: '--format playwright',
  geo: '--geo us',
  id: '--id oya-k3f9',
  'idempotency-key': '--idempotency-key checkout-42',
  key: '--key oya_...',
  label: '--label ci',
  locale: '--locale en-US',
  max: '--max 3',
  name: '--name shopper',
  out: '--out cookies.json',
  persona: '--persona auto',
  platform: '--platform MacIntel',
  policy: `--policy '{"allowedHosts":["example.com"]}'`,
  priority: '--priority normal',
  provider: '--provider cdp',
  'queue-ms': '--queue-ms 30000',
  role: '--role operator',
  tz: '--tz Europe/Berlin',
  url: '--url http://127.0.0.1:3100',
  'ws-url': '--ws-url ws://127.0.0.1:9222',
};

/** Splits argv (without node and the script) into command, arguments and flags; a flag it does not know throws. */
export function parse(argv: string[]): Invocation {
  const [command = 'help', ...rest] = argv;
  const args: string[] = [];
  const flags: Flags = {};
  for (let i = 0; i < rest.length; i++) {
    if (!rest[i].startsWith('--')) args.push(rest[i]);
    else if (takeFlag(rest[i].slice('--'.length), rest[i + 1], flags)) i++;
  }
  return { command, args, flags };
}

/** Records one flag; returns true when it consumed the next token as its value. */
function takeFlag(token: string, next: string | undefined, flags: Flags): boolean {
  const [name, inline] = splitOnce(token);
  // `--all=false` read as `--all` once stopped every browser: a switch given a value is refused.
  if (SWITCHES.has(name) && inline !== undefined) throw usage(`--${name} takes no value.`);
  flags[name] = SWITCHES.has(name) || valueOf(name, inline, next);
  return !SWITCHES.has(name) && inline === undefined;
}

/** A value flag's value: after its "=", else the next word; a flag nobody knows, or one missing its value, throws. */
function valueOf(name: string, inline: string | undefined, next: string | undefined): string {
  if (!Object.hasOwn(VALUE_FLAGS, name)) throw usage(`Unknown flag --${name}. Run oya help for the flags.`);
  if (inline !== undefined) return inline;
  if (next === undefined || next.startsWith('--'))
    throw usage(`--${name} needs a value, such as ${VALUE_FLAGS[name]}.`);
  return next;
}

/** `name=value` split at its first "=", or the name alone. */
function splitOnce(token: string): [string, string | undefined] {
  const at = token.indexOf('=');
  return at < 0 ? [token, undefined] : [token.slice(0, at), token.slice(at + 1)];
}

/** A flag's value, when it was given one. */
export const flagStr = (flags: Flags, name: string): string | undefined =>
  typeof flags[name] === 'string' ? (flags[name] as string) : undefined;

/** A flag's value as a number, when it was given one; a value that is not a number is a usage error. */
export function flagNum(flags: Flags, name: string): number | undefined {
  const raw = flagStr(flags, name);
  if (raw === undefined) return undefined;
  if (raw.trim() === '' || !Number.isFinite(Number(raw))) throw usage(`--${name} must be a number, not "${raw}".`);
  return Number(raw);
}

/**
 * A flag's value as a whole number of 1 or more; `none` too when `allowNone`,
 * which a persona's --max takes to lift its cap. Anything else is a usage error.
 */
export function flagCount(flags: Flags, name: string, { allowNone = false } = {}): number | null | undefined {
  const raw = flagStr(flags, name);
  if (raw === undefined) return undefined;
  if (allowNone && raw === 'none') return null;
  if (/^[1-9]\d*$/.test(raw)) return Number(raw);
  throw usage(`--${name} must be a whole number of 1 or more${allowNone ? ', or none' : ''}, not "${raw}".`);
}

/** `text` parsed as JSON, or an invalid_json error naming `subject` ("is", or "are" for a plural one) and how to quote it. */
export function parseJson(text: string, subject: string, hint?: string, verb = 'is'): unknown {
  try {
    return JSON.parse(text);
  } catch (err) {
    throw invalidJson(`${subject} ${verb} not valid JSON: ${(err as Error).message}.`, hint);
  }
}

/** A file's JSON, or a usage error naming the file: one that cannot be read, or does not parse, is the command's mistake. */
export function readJsonFile(file: string): unknown {
  let text: string;
  try {
    text = readFileSync(file, 'utf8');
  } catch (err) {
    throw usage(`${file} could not be read: ${(err as NodeJS.ErrnoException).code ?? (err as Error).message}.`);
  }
  return parseJson(text, file);
}
