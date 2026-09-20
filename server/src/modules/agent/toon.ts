/**
 * A small writer for TOON (Token-Oriented Object Notation, toonformat.dev):
 * uniform rows as one table that names its columns once, so the model reads a
 * page's elements with far fewer tokens than JSON or repeated labels, and the
 * [N] row count shows when a list was cut short.
 */

import pageRender from '../../../../browser/scripts/page-render.cjs';

/** One cell: numbers as they are, strings quoted (JSON style) only when TOON needs it. */
export const toonCell: (value: unknown) => string = pageRender.toonCell;

/** A tabular array: `name[rows]{a,b}:` then one indented row per item. */
export const toonTable: (name: string, columns: string[], rows: Record<string, unknown>[]) => string =
  pageRender.toonTable;
