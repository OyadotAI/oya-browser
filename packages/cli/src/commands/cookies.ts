/**
 * `oya cookies`: a persona's logins, moved to where they are needed. `export`
 * to a file or a script, `import` from one, `copy` between personas, `clear`.
 * Subcommands dispatch through a command map.
 */
import { chmodSync, writeFileSync } from 'node:fs';
import type { Cookie, CookieFormat, Oya } from '@oya-ai/browser';
import { flagStr, readJsonFile, type Flags } from '../args.ts';
import { usage } from '../errors.ts';
import { client, out } from '../context.ts';
import { JSON_INDENT, PRIVATE_FILE_MODE } from '../constants.ts';

/** A cookies subcommand: the client, the arguments after the subcommand, and the flags. */
type Subcommand = (oya: Oya, rest: string[], flags: Flags) => Promise<void>;

/** The formats a jar can be exported in. */
const FORMATS: readonly CookieFormat[] = ['json', 'playwright'];

/** The `--format` asked for, `json` by default. A typo is refused here, before the server is asked and before any file is touched. */
function formatOf(flags: Flags): CookieFormat {
  const format = flagStr(flags, 'format') || 'json';
  if (FORMATS.includes(format as CookieFormat)) return format as CookieFormat;
  throw usage(`Unknown format "${format}". Use --format json or --format playwright.`);
}

/** Writes the jar where only its owner can read it, a file that was already there included: it holds sessions. */
function writeJar(file: string, cookies: Cookie[]): void {
  writeFileSync(file, JSON.stringify(cookies, null, JSON_INDENT) + '\n', { mode: PRIVATE_FILE_MODE });
  chmodSync(file, PRIVATE_FILE_MODE); // an existing file keeps its old mode without this
}

/** `oya cookies export <persona>`: the jar on stdout, or in `--out`. Nothing is written unless the server sent a list. */
const exportJar: Subcommand = async (oya, rest, flags) => {
  if (!rest[0]) throw usage('oya cookies export needs a persona id: oya cookies export <persona> [--out <file>].');
  const cookies = await oya.personas.cookies(rest[0], formatOf(flags));
  if (!Array.isArray(cookies)) throw new Error('The server sent no cookie list, so nothing was written.');
  const file = flagStr(flags, 'out');
  if (!file) return console.log(JSON.stringify(cookies, null, JSON_INDENT));
  writeJar(file, cookies);
  const written = `✅ ${cookies.length} cookie${cookies.length === 1 ? '' : 's'} written to ${file}`;
  out(flags, { ok: true, id: rest[0], file, count: cookies.length }, () => console.log(written));
};

/** What a cookie file may hold: the list itself, or an export that wraps it. */
type CookieFile =
  | Cookie[]
  | {
      /** The wrapped list. */
      cookies?: unknown;
    };

/** The cookies in a file: a list, or an export wrapped as `{ cookies: [...] }`. */
function cookiesIn(file: string): Cookie[] {
  const parsed = readJsonFile(file) as CookieFile;
  const list = Array.isArray(parsed) ? parsed : parsed?.cookies;
  if (!Array.isArray(list)) throw usage(`${file} holds no list of cookies.`);
  return list;
}

/** `oya cookies import <persona> <file.json>`. */
const importJar: Subcommand = async (oya, rest, flags) => {
  if (!rest[0] || !rest[1])
    throw usage('oya cookies import needs a persona and a file: oya cookies import <persona> <file>.');
  const done = await oya.personas.importCookies(rest[0], cookiesIn(rest[1]));
  out(flags, done, () =>
    console.log(`✅ ${done.imported} imported, ${done.skipped} skipped, ${done.total} in the jar`),
  );
};

/** `oya cookies copy <from> <to>`: the second persona keeps its own device and gains the first one's logins. */
const copy: Subcommand = async (oya, rest, flags) => {
  if (!rest[0] || !rest[1]) throw usage('oya cookies copy needs two persona ids: oya cookies copy <from> <to>.');
  const done = await oya.personas.copyCookies(rest[0], rest[1]);
  out(flags, done, () => console.log(`✅ ${done.imported} copied to ${rest[1]}, ${done.total} in its jar`));
};

/** `oya cookies clear <persona>`: signs the persona out everywhere. */
const clear: Subcommand = async (oya, rest, flags) => {
  if (!rest[0]) throw usage('oya cookies clear needs a persona id: oya cookies clear <persona>.');
  const done = await oya.personas.clearCookies(rest[0]);
  out(flags, done ?? { ok: true, id: rest[0] }, () => console.log(`✅ cleared ${rest[0]}`));
};

/** Subcommands by name. */
const SUBCOMMANDS: Record<string, Subcommand> = { export: exportJar, import: importJar, copy, clear };

/** `oya cookies <export|import|copy|clear>`. */
export async function cmdCookies(args: string[], flags: Flags): Promise<void> {
  const [sub, ...rest] = args;
  if (sub === undefined || !Object.hasOwn(SUBCOMMANDS, sub))
    throw usage(`Unknown cookies subcommand "${sub ?? ''}". Use export, import, copy or clear.`);
  await SUBCOMMANDS[sub](client(flags), rest, flags);
}
