/** Measure terminal cells and wrap setup prose without truncating identifiers or copyable commands. */
import { stdout } from 'node:process';
import { ESC } from './style.ts';
import {
  DEFAULT_COLUMNS,
  SETUP_CONTENT_COLUMNS,
  SETUP_INSET,
  TAB_COLUMNS,
  PRINTABLE_START,
  DELETE_CODE,
  WIDE_CELL_WIDTH,
} from './constants.ts';

/** ANSI decorations do not occupy cells. */
const ANSI = new RegExp(`${ESC}\\[[0-9;]*m`, 'g');
/** Combining marks and joiners do not advance the cursor. */
const ZERO_WIDTH = /(?:\p{Mark}|\u200d|\ufe0e|\ufe0f)/u;
/** Common East Asian and emoji characters occupy two cells in a terminal. */
const WIDE =
  /[\u1100-\u115f\u2329\u232a\u2e80-\ua4cf\uac00-\ud7a3\uf900-\ufaff\ufe10-\ufe6f\uff01-\uff60\uffe0-\uffe6\u{1f300}-\u{1faff}]/u;

/** Emoji sequences form one displayed grapheme even when several code points are joined. */
const EMOJI = /(?:\p{Emoji_Presentation}|\p{Regional_Indicator}|\u20e3|\ufe0f)/u;
/** Standard grapheme segmentation keeps cursor accounting dependency-free. */
const GRAPHEMES = new Intl.Segmenter(undefined, { granularity: 'grapheme' });

/** Ignore terminal control characters when displaying values, without changing the actual answer. */
export const displayText = (value: string): string =>
  Array.from(value.replace(ANSI, ''))
    .filter((char) => !controlCharacter(char))
    .join('');

/** Control bytes are not safe display content, even when an answer itself is left unchanged. */
function controlCharacter(char: string): boolean {
  const code = char.charCodeAt(0);
  return code < PRINTABLE_START || code === DELETE_CODE;
}

/** Count cells rather than UTF-16 units so Unicode paths do not corrupt cursor accounting. */
export function cellWidth(value: string): number {
  return Array.from(GRAPHEMES.segment(value.replace(ANSI, '')), (part) => part.segment).reduce((width, char) => {
    if (char === '\t') return width + TAB_COLUMNS - (width % TAB_COLUMNS);
    if (Array.from(char).every((part) => ZERO_WIDTH.test(part)) || controlCharacter(char)) return width;
    return width + (WIDE.test(char) || EMOJI.test(char) ? WIDE_CELL_WIDTH : 1);
  }, 0);
}

/** Content fits both the rail and the actual terminal. */
export const contentColumns = (): number =>
  Math.max(1, Math.min((stdout.columns || DEFAULT_COLUMNS) - SETUP_INSET, SETUP_CONTENT_COLUMNS));

/** An oversized word remains intact and is allowed to soft-wrap in the terminal. */
export function wrapProse(value: string, columns = contentColumns()): string[] {
  const lines: string[] = [];
  for (const word of displayText(value).split(/\s+/).filter(Boolean)) {
    const last = lines.at(-1);
    if (!last || cellWidth(`${last} ${word}`) > columns) lines.push(word);
    else lines[lines.length - 1] += ` ${word}`;
  }
  return lines.length ? lines : [''];
}

/** Physical terminal rows include native wrapping of long identifiers and commands. */
export const terminalRows = (value: string, columns = stdout.columns || DEFAULT_COLUMNS): number =>
  Math.max(1, Math.ceil(cellWidth(value) / columns));

/** Stack long values below their label while retaining every character. */
export function labelValue(label: string, value: string): string[] {
  const safe = displayText(value),
    line = `${label}  ${safe}`;
  return cellWidth(line) <= contentColumns() ? [line] : [label, `  ${safe}`];
}
