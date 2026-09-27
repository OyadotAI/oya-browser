/**
 * Where the licensing module keeps its few values, such as the install id:
 * rows of the settings table, as JSON.
 */
import { getConnection } from '../storage/index.ts';

/** A value the licensing module stored under `name`, or null. */
export async function get(name: string) {
  const [row] = await getConnection().select('settings', { key: `license:${name}` });
  return row ? JSON.parse(String(row.value)) : null;
}

/** Stores a value under `name`. */
export async function set(name: string, value: unknown) {
  const row = { key: `license:${name}`, value: JSON.stringify(value), updated_at: new Date().toISOString() };
  await getConnection().upsert('settings', [row], { update: true });
}
