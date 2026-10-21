/**
 * What every command needs: an SDK client for the right key and control
 * plane, the browser to act on, and output as JSON or for people.
 */
import { Oya, type Browser } from '@oya-ai/browser';
import { origin as originOf, type Origin } from './config.ts';
import { flagStr, type Flags } from './args.ts';
import { CliError } from './errors.ts';
import { JSON_INDENT } from './constants.ts';

/** The key and address this call uses, from `--key`/`--url`, else the environment, else the saved config. */
export const origin = (flags: Flags): Origin =>
  originOf({ apiKey: flagStr(flags, 'key'), baseUrl: flagStr(flags, 'url') });

/** A client for this call's key and address. Throws without a key: nothing can be sent. */
export function client(flags: Flags): Oya {
  const { apiKey, baseUrl, savedKey, savedUrl } = origin(flags);
  if (!apiKey) throw new CliError('No API key.', 'no_api_key', { hint: noKeyHint(baseUrl, savedKey, savedUrl) });
  return new Oya({ apiKey, baseUrl });
}

/**
 * What to do without a key. A saved key that went unused is one `oya login` saved
 * for another control plane; the hint names it, and offers --key and OYA_API_KEY
 * before `oya login --url`, which would replace the saved key.
 */
function noKeyHint(baseUrl: string, savedKey: boolean, savedUrl?: string): string {
  if (!savedKey) return 'Run oya login, or pass --key, or set OYA_API_KEY.';
  const why = `The key oya login saved is for ${savedUrl ?? 'another control plane'} and is only sent there.`;
  return `${why} Pass --key or set OYA_API_KEY for ${baseUrl}, or run oya login --url ${baseUrl} to replace the saved key.`;
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
  if (!all.length) throw new CliError('No browsers running.', 'no_browser', { hint: 'Start one with oya start.' });
  return oya.browser.get(all[all.length - 1].id);
}
