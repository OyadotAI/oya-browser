/**
 * Account commands: `oya config`, `oya usage` and `oya whoami`.
 */
import { configPath } from '../config.ts';
import type { Flags } from '../args.ts';
import { client, origin, out, printJson } from '../context.ts';
import { KEY_TAIL, MIN_KEY_FOR_TAIL } from '../constants.ts';
import { usage } from '../errors.ts';

/** `key=value` pairs as config updates; anything else is an error. */
function parsePairs(args: string[]): Record<string, string> {
  const updates: Record<string, string> = {};
  for (const pair of args) {
    const index = pair.indexOf('=');
    if (index < 1) throw usage(`oya config takes settings as key=value, not "${pair}".`);
    updates[pair.slice(0, index)] = pair.slice(index + 1);
  }
  return updates;
}

/** `oya config [key=value ...]`: show or change this key's settings. */
export async function cmdConfig(args: string[], flags: Flags): Promise<void> {
  const oya = client(flags);
  if (!args.length) {
    const current = await oya.config.get();
    return out(flags, current, () => printJson(current));
  }
  const updates = parsePairs(args);
  const saved = await oya.config.set(updates);
  out(flags, saved, () => console.log(`✅ updated ${Object.keys(updates).join(', ')}`));
}

/** `oya usage`: what this key has spent. */
export async function cmdUsage(flags: Flags): Promise<void> {
  printJson(await client(flags).usage());
}

/** `oya whoami`: the control plane in use, and whether a key is saved (never the key). */
export async function cmdWhoami(flags: Flags): Promise<void> {
  const { apiKey, apiKeyFrom, baseUrl, baseUrlFrom, savedKey } = origin(flags);
  const tail = apiKey.length >= MIN_KEY_FOR_TAIL ? { apiKeyEnds: apiKey.slice(-KEY_TAIL) } : {};
  const ignored = savedKey && apiKeyFrom !== 'saved' ? { savedKeyIgnored: true } : {};
  printJson({ baseUrl, baseUrlFrom, apiKeyFrom, ...tail, ...ignored, configPath });
}
