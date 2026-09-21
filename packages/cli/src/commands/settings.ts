/**
 * Account commands: `oya config`, `oya usage` and `oya whoami`.
 */
import { load, resolved } from '../config.ts';
import type { Flags } from '../args.ts';
import { client, out, printJson } from '../context.ts';

/** `key=value` pairs as config updates; anything else is an error. */
function parsePairs(args: string[]): Record<string, string> {
  const updates: Record<string, string> = {};
  for (const pair of args) {
    const index = pair.indexOf('=');
    if (index < 1) throw new Error(`Expected key=value, got "${pair}"`);
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
  await oya.config.set(updates);
  console.log(`✅ updated ${Object.keys(updates).join(', ')}`);
}

/** `oya usage`: what this key has spent. */
export async function cmdUsage(flags: Flags): Promise<void> {
  printJson(await client(flags).usage());
}

/** `oya whoami`: the control plane in use, and whether a key is saved (never the key). */
export async function cmdWhoami(): Promise<void> {
  printJson({ ...resolved(), apiKey: load().apiKey ? 'saved' : 'none' });
}
