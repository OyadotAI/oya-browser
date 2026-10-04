/**
 * Loads audit rows the server spilled to its overflow file
 * (data/audit-overflow.jsonl, when storage could not keep up) into audit_log.
 *
 *   node src/platform/audit-import.ts [file]
 *
 * Safe to run again: a row whose chain position is already stored is skipped,
 * so a rerun after a partial import, or on a file imported before, adds only
 * what is missing. The rows keep their chain links, so once imported the
 * chain verifies without a gap. The file is left in place; remove it once the
 * import reports every row imported or skipped.
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { OVERFLOW_FILE } from './audit.ts';
import { closeConnection, getConnection } from './storage/index.ts';

/** A row's place in its chain, as one string. */
const position = (row) => `${row.chain}#${row.seq}`;

/** Rows in the overflow file, one JSON object per line, each position once. */
function readRows(file: string) {
  const lines = readFileSync(file, 'utf8')
    .split('\n')
    .filter((line) => line.trim());
  const rows = new Map(lines.map((line) => JSON.parse(line)).map((row) => [position(row), row]));
  return [...rows.values()];
}

/** Positions already stored for the chains among `rows`, from the lowest position the file holds. */
async function storedPositions(rows): Promise<Set<string>> {
  const lowest = new Map();
  for (const r of rows) lowest.set(r.chain, Math.min(lowest.get(r.chain) ?? r.seq, r.seq));
  const reads = [...lowest].map(([chain, seq]) => getConnection().select('audit_log', { chain, seq: { gte: seq } }));
  return new Set((await Promise.all(reads)).flat().map(position));
}

/** What an import did. */
export type ImportResult = {
  /** Distinct rows in the file. */
  read: number;
  /** Rows written to audit_log. */
  imported: number;
  /** Rows already stored, left alone. */
  skipped: number;
};

/** Imports the overflow file's rows that audit_log does not already hold. */
export async function importOverflow(file = OVERFLOW_FILE): Promise<ImportResult> {
  const rows = readRows(file);
  const stored = await storedPositions(rows);
  const fresh = rows.filter((row) => !stored.has(position(row)));
  if (fresh.length) await getConnection().upsert('audit_log', fresh);
  return { read: rows.length, imported: fresh.length, skipped: rows.length - fresh.length };
}

/** Runs the import from the command line and says what it did. */
async function main(file?: string) {
  try {
    console.log(`[audit] import: ${JSON.stringify(await importOverflow(file || OVERFLOW_FILE))}`);
  } catch (e) {
    console.error(`[audit] import failed: ${e.message}`);
    process.exitCode = 1;
  } finally {
    await closeConnection();
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) await main(process.argv[2]);
