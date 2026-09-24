/**
 * Data files from before storage had drivers (personas.json,
 * key-settings.json, config.json). Each is read once into whichever driver is
 * configured and then renamed to <name>.imported, so nothing is lost on upgrade
 * and nothing is imported twice. The file is kept, never deleted.
 */
import { readFile, rename } from 'node:fs/promises';

/** Reads `path` as JSON and hands it to `load`, then sets the file aside; nothing when there is no file. */
export async function importLegacyFile(path: string, load: (data: any) => Promise<unknown>) {
  const data = await readJson(path);
  if (data === undefined) return;
  await load(data);
  await rename(path, `${path}.imported`);
  console.log(`[storage] imported ${path}`);
}

/** A file's JSON, or undefined when there is no file. */
async function readJson(path: string) {
  try {
    return JSON.parse(await readFile(path, 'utf8'));
  } catch (e) {
    if (e.code === 'ENOENT') return undefined;
    throw e;
  }
}
