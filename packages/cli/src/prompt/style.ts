/**
 * Colour and cursor control for the terminal. On a pipe, with NO_COLOR, or on
 * a dumb terminal, every style is the identity and nothing moves the cursor.
 */
import { stdout } from 'node:process';
import { DEFAULT_COLUMNS, MAX_RULE_WIDTH } from './constants.ts';

/** The escape character that opens every ANSI sequence. */
export const ESC = '\u001b';

/** True when output must stay free of escape codes. */
export const plain = !stdout.isTTY || !!process.env.NO_COLOR || process.env.TERM === 'dumb';

/** A style that wraps text in one SGR code, or leaves it alone when plain. */
const wrap = (open: string) => (s: string) => (plain ? s : `${ESC}[${open}m${s}${ESC}[0m`);

/** The colours and weights the CLI uses. */
export const style = {
  bold: wrap('1'),
  dim: wrap('2'),
  green: wrap('32'),
  cyan: wrap('36'),
  yellow: wrap('33'),
  red: wrap('31'),
  grey: wrap('90'),
};

/** Status marks, already coloured. */
export const icon = {
  ok: style.green('✔'),
  fail: style.red('✗'),
  warn: style.yellow('!'),
  arrow: style.green('❯'),
};

/** Clears from the cursor to the end of the line. */
export const clearLine = plain ? '' : `${ESC}[K`;

/** Moves the cursor up `n` rows. */
export const up = (n: number) => `${ESC}[${n}A`;

/** Width of a banner rule: the terminal's, capped. */
export const width = () => Math.min(stdout.columns || DEFAULT_COLUMNS, MAX_RULE_WIDTH);

/** Matches a colour code, which takes no columns. */
const COLOUR_CODE = new RegExp(`${ESC}\\[[0-9;]*m`, 'g');

/** Printable width, ignoring the colour codes that take no columns. */
export const visible = (text: string) => text.replace(COLOUR_CODE, '').length;
