/**
 * What every command needs: an SDK client for the right key and control
 * plane, the browser to act on, and output as JSON or for people.
 */
import { Oya, type Browser } from '@oya-ai/browser';
import { resolved } from './config.ts';
import { flagStr, type Flags } from './args.ts';
import { JSON_INDENT } from './constants.ts';

/** A client for `--key`/`--url`, else the environment, else the saved config. Exits without a key. */
export function client(flags: Flags): Oya {
  const saved = resolved();
  const apiKey = flagStr(flags, 'key') || saved.apiKey;
  const baseUrl = flagStr(flags, 'url') || saved.baseUrl;
  if (!apiKey) {
    console.error('No API key. Run `oya login`, or pass --key / set OYA_API_KEY.');
    process.exit(1);
  }
  return new Oya({ apiKey, baseUrl });
}

/** Prints a value as indented JSON. */
export const printJson = (value: unknown): void => console.log(JSON.stringify(value, null, JSON_INDENT));

/** `--json` prints the value; otherwise `human` prints it for people. */
export const out = (flags: Flags, value: unknown, human: () => void): void => {
  if (flags.json) printJson(value);
  else human();
};

/** The browser a command acts on when none is named: the most recent one. */
export async function targetBrowser(oya: Oya, flags: Flags): Promise<Browser> {
  const id = flagStr(flags, 'id');
  if (id) return oya.browser.get(id);
  const all = await oya.browser.list();
  if (!all.length) {
    console.error('No browsers running. Start one with `oya start`.');
    process.exit(1);
  }
  return oya.browser.get(all[all.length - 1].id);
}
