/**
 * `oya cookies`: a persona's logins, moved to where they are needed. `export`
 * to a file or a script, `import` from one, `copy` between personas, `clear`.
 * Subcommands dispatch through a command map.
 */
import { readFileSync, writeFileSync } from 'node:fs';
import type { Cookie, CookieFormat, Oya } from '@oya-ai/browser';
import { flagStr, type Flags } from '../args.ts';
import { client, out } from '../context.ts';
import { JSON_INDENT, PRIVATE_FILE_MODE } from '../constants.ts';

/** A cookies subcommand: the client, the arguments after the subcommand, and the flags. */
type Subcommand = (oya: Oya, rest: string[], flags: Flags) => Promise<void>;

/** How the command is used, shown when it is not. */
const USAGE = `Usage: oya cookies export <persona> [--format json|playwright] [--out <file>]
       oya cookies import <persona> <file.json>
       oya cookies copy <from-persona> <to-persona>
       oya cookies clear <persona>`;

/** `oya cookies export <persona>`: the jar on stdout, or in `--out`, a file only its owner can read: it holds sessions. */
const exportJar: Subcommand = async (oya, rest, flags) => {
  if (!rest[0]) throw new Error('Usage: oya cookies export <persona> [--format json|playwright] [--out <file>]');
  const cookies = await oya.personas.cookies(rest[0], (flagStr(flags, 'format') || 'json') as CookieFormat);
  const file = flagStr(flags, 'out');
  if (!file) return console.log(JSON.stringify(cookies, null, JSON_INDENT));
  writeFileSync(file, JSON.stringify(cookies, null, JSON_INDENT) + '\n', { mode: PRIVATE_FILE_MODE });
  console.log(`✅ ${cookies.length} cookie${cookies.length === 1 ? '' : 's'} written to ${file}`);
};

/** The cookies in a file: a list, or an export wrapped as `{ cookies: [...] }`. */
function cookiesIn(file: string): Cookie[] {
  const parsed = JSON.parse(readFileSync(file, 'utf8'));
  const list = Array.isArray(parsed) ? parsed : parsed?.cookies;
  if (!Array.isArray(list)) throw new Error(`${file} holds no list of cookies`);
  return list;
}

/** `oya cookies import <persona> <file.json>`. */
const importJar: Subcommand = async (oya, rest, flags) => {
  if (!rest[0] || !rest[1]) throw new Error('Usage: oya cookies import <persona> <file.json>');
  const done = await oya.personas.importCookies(rest[0], cookiesIn(rest[1]));
  out(flags, done, () =>
    console.log(`✅ ${done.imported} imported, ${done.skipped} skipped, ${done.total} in the jar`),
  );
};

/** `oya cookies copy <from> <to>`: the second persona keeps its own device and gains the first one's logins. */
const copy: Subcommand = async (oya, rest, flags) => {
  if (!rest[0] || !rest[1]) throw new Error('Usage: oya cookies copy <from-persona> <to-persona>');
  const done = await oya.personas.copyCookies(rest[0], rest[1]);
  out(flags, done, () => console.log(`✅ ${done.imported} copied to ${rest[1]}, ${done.total} in its jar`));
};

/** `oya cookies clear <persona>`: signs the persona out everywhere. */
const clear: Subcommand = async (oya, rest) => {
  if (!rest[0]) throw new Error('Usage: oya cookies clear <persona>');
  await oya.personas.clearCookies(rest[0]);
  console.log(`✅ cleared ${rest[0]}`);
};

/** Subcommands by name. */
const SUBCOMMANDS: Record<string, Subcommand> = { export: exportJar, import: importJar, copy, clear };

/** `oya cookies <export|import|copy|clear>`. */
export async function cmdCookies(args: string[], flags: Flags): Promise<void> {
  const [sub, ...rest] = args;
  if (sub === undefined || !Object.hasOwn(SUBCOMMANDS, sub)) throw new Error(USAGE);
  await SUBCOMMANDS[sub](client(flags), rest, flags);
}
