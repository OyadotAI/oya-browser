/**
 * A small writer for TOON (Token-Oriented Object Notation, toonformat.dev):
 * uniform rows as one table that names its columns once, so the model reads a
 * page's elements with far fewer tokens than JSON or repeated labels, and the
 * [N] row count shows when a list was cut short.
 */

/** A cell that must be quoted: empty, padded, a keyword or number, or carrying TOON syntax. */
const NEEDS_QUOTES = /^$|^\s|\s$|^(true|false|null)$|^-?\d+(\.\d+)?(e[+-]?\d+)?$|[:,"\\[\]{}\n\r\t]|^-|^#$/i;

/** One cell: numbers as they are, strings quoted (JSON style) only when TOON needs it. */
export function toonCell(value: unknown): string {
  if (typeof value === 'number' && Number.isFinite(value)) return String(value);
  const text = value == null ? '' : String(value);
  return NEEDS_QUOTES.test(text) ? JSON.stringify(text) : text;
}

/** A tabular array: `name[rows]{a,b}:` then one indented row per item. */
export function toonTable(name: string, columns: string[], rows: Record<string, unknown>[]): string {
  const header = `${name}[${rows.length}]{${columns.join(',')}}:`;
  return [header, ...rows.map((row) => '  ' + columns.map((c) => toonCell(row[c])).join(','))].join('\n');
}
