/**
 * `oya playbooks`: saved playbooks, and moving them between environments. `export`
 * one to a file on one server, `import` it on another with its key. Secrets travel
 * by name only. Subcommands dispatch through a command map.
 */
import { writeFileSync } from 'node:fs';
import type { Oya, PlaybookExport } from '@oya-ai/browser';
import { flagStr, readJsonFile, type Flags } from '../args.ts';
import { usage } from '../errors.ts';
import { client, out } from '../context.ts';
import { JSON_INDENT } from '../constants.ts';

/** A playbooks subcommand: the client, the arguments after the subcommand, and the flags. */
type Subcommand = (oya: Oya, rest: string[], flags: Flags) => Promise<void>;

/** `oya playbooks list`: each playbook with its step count and inputs. */
const list: Subcommand = async (oya, _rest, flags) => {
  const playbooks = await oya.playbooks.list();
  out(flags, playbooks, () => {
    if (!playbooks.length) return console.log('No playbooks yet: save a run with browser.toPlaybook().');
    for (const p of playbooks) console.log(`${p.name}  ${p.steps} steps  ${p.variables.join(', ') || '(no inputs)'}`);
  });
};

/** `oya playbooks export <name>`: the export on stdout, or in `--out`. */
const exportOne: Subcommand = async (oya, rest, flags) => {
  if (!rest[0]) throw usage('oya playbooks export needs a name: oya playbooks export <name> [--out <file>].');
  const doc = await oya.playbooks.export(rest[0]);
  const file = flagStr(flags, 'out');
  if (!file) return console.log(JSON.stringify(doc, null, JSON_INDENT));
  writeFileSync(file, JSON.stringify(doc, null, JSON_INDENT) + '\n');
  out(flags, { ok: true, name: rest[0], file }, () => console.log(`✅ ${rest[0]} written to ${file}`));
};

/** `oya playbooks import <file>`: saved under `--name`, or the name it was exported with. */
const importOne: Subcommand = async (oya, rest, flags) => {
  if (!rest[0])
    throw usage('oya playbooks import needs a file: oya playbooks import <file> [--name <name>] [--replace].');
  const doc = readJsonFile(rest[0]) as PlaybookExport;
  const name = flagStr(flags, 'name') || undefined;
  const saved = await oya.playbooks.import(doc, { name, overwrite: flags.replace === true });
  out(flags, saved, () => console.log(`✅ imported ${saved.name}: ${saved.steps} steps`));
};

/** Subcommands by name. */
const SUBCOMMANDS: Record<string, Subcommand> = { list, export: exportOne, import: importOne };

/** `oya playbooks <list|export|import>`. */
export async function cmdPlaybooks(args: string[], flags: Flags): Promise<void> {
  const [sub = 'list', ...rest] = args;
  if (!Object.hasOwn(SUBCOMMANDS, sub))
    throw usage(`Unknown playbooks subcommand "${sub}". Use list, export or import.`);
  await SUBCOMMANDS[sub](client(flags), rest, flags);
}
