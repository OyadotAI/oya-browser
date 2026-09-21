/**
 * `oya personas`: list identities, or `new`, `edit`, `clone` and `rm` them.
 * Subcommands dispatch through a command map; anything else lists.
 */
import type { Oya, PersonaInfo, PersonaPrefs } from '@oya-ai/browser';
import { flagStr, type Flags } from '../args.ts';
import { client, out } from '../context.ts';
import { PERSONA_NAME_WIDTH } from '../constants.ts';

/** A persona subcommand: the client, the arguments after the subcommand, and the flags. */
type Subcommand = (oya: Oya, rest: string[], flags: Flags) => Promise<void>;

/** Accepted spellings of each platform. */
const PLATFORM_ALIAS: Record<string, NonNullable<PersonaPrefs['platform']>> = {
  win32: 'Win32',
  windows: 'Win32',
  win: 'Win32',
  macintel: 'MacIntel',
  mac: 'MacIntel',
  macos: 'MacIntel',
  linux: 'Linux x86_64',
  'linux x86_64': 'Linux x86_64',
};

/** Device choices from `--platform`, `--tz` and `--locale`. */
function prefsFrom(flags: Flags): PersonaPrefs {
  const platformFlag = flagStr(flags, 'platform');
  return {
    ...(platformFlag ? { platform: PLATFORM_ALIAS[platformFlag.toLowerCase()] || (platformFlag as never) } : {}),
    ...(flagStr(flags, 'tz') ? { timezone: flagStr(flags, 'tz') } : {}),
    ...(flagStr(flags, 'locale') ? { locale: flagStr(flags, 'locale') } : {}),
  };
}

/** What `new` creates: name, device choices, geo hint and cap from the flags. */
function createOptions(flags: Flags, rest: string[], prefs: PersonaPrefs) {
  return {
    name: flagStr(flags, 'name') || rest[0],
    prefs,
    proxy: flagStr(flags, 'geo') ? { geo: flagStr(flags, 'geo') } : undefined,
    maxConcurrent: flagStr(flags, 'max') ? Number(flagStr(flags, 'max')) : undefined,
  };
}

/** `oya personas new [name]`, or with `--preview` just the fingerprint it would get. */
const create: Subcommand = async (oya, rest, flags) => {
  const prefs = prefsFrom(flags);
  if (flags.preview) return preview(oya, prefs, flags);
  const created = await oya.personas.create(createOptions(flags, rest, prefs));
  const fp = created.fingerprint;
  out(flags, created, () => console.log(`✅ ${created.id}  ${created.name}  ${fp.platform} · ${fp.timezone}`));
};

/** Prints the fingerprint some choices would produce. */
async function preview(oya: Oya, prefs: PersonaPrefs, flags: Flags): Promise<void> {
  const fp = await oya.personas.preview(prefs);
  out(flags, fp, () => console.log(`${fp.platform} · ${fp.timezone} · ${fp.locale} · ${fp.screen} · ${fp.webgl}`));
}

/** The changes `edit` makes: only the flags given. `--max none` removes the cap. */
function editChanges(flags: Flags) {
  const max = flagStr(flags, 'max');
  return {
    ...(flagStr(flags, 'name') ? { name: flagStr(flags, 'name') } : {}),
    ...(max ? { maxConcurrent: max === 'none' ? null : Number(max) } : {}),
    ...(flagStr(flags, 'geo') ? { proxy: { geo: flagStr(flags, 'geo') } } : {}),
  };
}

/** `oya personas edit <id>`. */
const edit: Subcommand = async (oya, rest, flags) => {
  if (!rest[0]) throw new Error('Usage: oya personas edit <id> --name <n> --max <n> --geo <cc>');
  const updated = await oya.personas.update(rest[0], editChanges(flags));
  out(flags, updated, () => console.log(`✅ ${updated.id}  ${updated.name}  cap ${updated.maxConcurrent ?? '∞'}`));
};

/** `oya personas clone <id>`: a new identity on the same kind of device. */
const clone: Subcommand = async (oya, rest, flags) => {
  if (!rest[0]) throw new Error('Usage: oya personas clone <id> [--name <n>]');
  const c = await oya.personas.clone(rest[0], { name: flagStr(flags, 'name') });
  const fp = c.fingerprint;
  out(flags, c, () =>
    console.log(`✅ ${c.id}  ${c.name}  ${fp.platform} · ${fp.timezone}  (new identity, same kind of device)`),
  );
};

/** `oya personas rm <id>`. */
const remove: Subcommand = async (oya, rest) => {
  if (!rest[0]) throw new Error('Usage: oya personas rm <id>');
  await oya.personas.remove(rest[0]);
  console.log(`✅ removed ${rest[0]}`);
};

/** One persona's line in the listing. */
function personaLine(p: PersonaInfo): string {
  const cap = p.maxConcurrent === null ? '∞' : String(p.maxConcurrent);
  const where = p.exit ? `  via ${p.exit.label}` : p.proxy?.geo ? `  geo ${p.proxy.geo}` : '';
  const extras = `${where}${p.mfa?.configured ? `  mfa:${p.mfa.type}` : ''}${p.isDefault ? '  (default)' : ''}`;
  const device = `${p.fingerprint.platform} · ${p.fingerprint.timezone}`;
  return `${p.id}  ${p.name.padEnd(PERSONA_NAME_WIDTH)} ${p.activeBrowsers}/${cap} running  ${device}${extras}`;
}

/** `oya personas`: every identity and its concurrency. */
const list: Subcommand = async (oya, _rest, flags) => {
  const all = await oya.personas.list();
  out(flags, all, () => {
    if (!all.length) return console.log('No personas yet, `oya personas new`.');
    for (const p of all) console.log(personaLine(p));
  });
};

/** Subcommands by name; aliases share a handler. */
const SUBCOMMANDS: Record<string, Subcommand> = { new: create, create, edit, clone, rm: remove, delete: remove };

/** `oya personas [new|create|edit|clone|rm|delete]`. */
export async function cmdPersonas(args: string[], flags: Flags): Promise<void> {
  const oya = client(flags);
  const [sub, ...rest] = args;
  const run = sub !== undefined && Object.hasOwn(SUBCOMMANDS, sub) ? SUBCOMMANDS[sub] : list;
  await run(oya, rest, flags);
}
