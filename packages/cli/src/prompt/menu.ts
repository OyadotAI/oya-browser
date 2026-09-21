/**
 * Menus. On a TTY they are arrow-key selectable and redraw in place; anywhere
 * else they fall back to a numbered list read line by line, so scripted input
 * behaves identically to typing.
 *
 * `disabled` keeps a planned option visible without pretending it works: it is
 * listed, dimmed, and skipped over. A menu that silently omits what the docs
 * promise is more confusing than one that says "not yet".
 */
import { stdin, stdout } from 'node:process';
import { ESC, clearLine, icon, style, up, visible } from './style.ts';
import { InputError, ask } from './ask.ts';
import { pauseLines, resumeLines } from './lines.ts';
import { DEFAULT_COLUMNS, DEFAULT_ROWS } from './constants.ts';

/** One choice in a menu. */
export interface Option {
  /** What `choose()` returns when it is picked. */
  id: string;
  /** What the menu shows. */
  label: string;
  /** A dimmed hint after the label. */
  note?: string;
  /** Why it cannot be picked; set, the option is shown dimmed and skipped. */
  disabled?: string;
}

/** An interactive menu's state while it is open. */
interface Menu {
  /** The options, as given. */
  options: Option[];
  /** Indexes of the options that can be picked. */
  pickable: number[];
  /** The highlighted option. */
  active: number;
  /** Terminal rows the menu takes, hint included. */
  rows: number;
  /** Whether the whole menu is still on screen, so it can be redrawn in place. */
  canRedraw: boolean;
}

/** What a key press does to an open menu; settles the menu by returning an outcome. */
type KeyAction = (menu: Menu) => Outcome | void;
/** How a menu ended: an option's id, or null for a cancel (Ctrl+C). */
interface Outcome {
  /** The chosen option's id, or null when cancelled. */
  id: string | null;
}

/** The hint under an interactive menu. */
const hint = () => style.grey('  ↑↓ move · enter select');

/** One option's line. */
function optionLine(o: Option, i: number, chosen: boolean, interactive: boolean): string {
  const bullet = chosen ? icon.arrow : ' ';
  const number = interactive ? '' : style.grey(`${i + 1}) `);
  const note = o.disabled ? `— ${o.disabled}` : o.note ? `— ${o.note}` : '';
  const label = o.disabled ? style.grey(o.label) : chosen ? style.green(o.label) : o.label;
  return `  ${bullet} ${number}${label}${note ? style.grey(`  ${note}`) : ''}`;
}

/**
 * Returns the number of terminal ROWS drawn, which is not the number of lines: a
 * label longer than the window wraps. Redrawing in place means moving the cursor
 * up by rows, and counting lines instead leaves the previous menu on screen.
 */
function renderOptions(options: Option[], active: number, interactive: boolean): number {
  const columns = stdout.columns || DEFAULT_COLUMNS;
  let rows = 0;
  options.forEach((o, i) => {
    const line = optionLine(o, i, interactive && i === active, interactive);
    stdout.write(`${line}${clearLine}\n`);
    rows += Math.max(1, Math.ceil(visible(line) / columns));
  });
  return rows;
}

/** Draws the menu again over itself, if it is still on screen. */
function redraw(menu: Menu): void {
  if (!menu.canRedraw) return;
  stdout.write(up(menu.rows));
  menu.rows = renderOptions(menu.options, menu.active, true) + 1;
  stdout.write(`${hint()}\n`);
}

/** Moves the highlight by `delta` pickable options, wrapping. */
const move =
  (delta: number): KeyAction =>
  (menu) => {
    const here = menu.pickable.indexOf(menu.active);
    menu.active = menu.pickable[(here + delta + menu.pickable.length) % menu.pickable.length];
    redraw(menu);
  };

/** A number key jumps to that option, but not onto a disabled one. */
const jump =
  (key: string): KeyAction =>
  (menu) => {
    const target = Number(key) - 1;
    if (menu.options[target] && !menu.options[target].disabled) {
      menu.active = target;
      redraw(menu);
    }
  };

/** Enter picks the highlighted option. */
const select: KeyAction = (menu) => ({ id: menu.options[menu.active].id });

/** What each key does. Digits are handled by `jump`. */
const KEYS: Record<string, KeyAction> = {
  '\u0003': () => ({ id: null }),
  [`${ESC}[A`]: move(-1),
  k: move(-1),
  [`${ESC}[B`]: move(1),
  j: move(1),
  '\r': select,
  '\n': select,
};

/** The action for a key press, if it has one. */
function actionFor(key: string): KeyAction | undefined {
  if (Object.hasOwn(KEYS, key)) return KEYS[key];
  return /^[1-9]$/.test(key) ? jump(key) : undefined;
}

/** Collapse the menu to just the chosen line, as a record of the answer. */
function collapse(menu: Menu, chosen: string): void {
  stdout.write(up(menu.rows));
  for (let i = 0; i < menu.rows; i++) stdout.write(`${clearLine}\n`);
  stdout.write(up(menu.rows));
  // The cursor is left directly under the chosen line; the rest of the
  // region is already blank, so the next prompt simply writes over it.
  stdout.write(`${chosen}${clearLine}\n`);
}

/** Leaves the chosen line on screen as a record of the answer. */
function recordChoice(menu: Menu): void {
  const chosen = `  ${icon.arrow} ${style.green(menu.options[menu.active].label)}`;
  if (menu.canRedraw) collapse(menu, chosen);
  else stdout.write(`${chosen}\n`);
}

/** The option a typed answer names: its number, or its id. */
const findOption = (options: Option[], v: string) => options[Number(v) - 1] || options.find((o) => o.id === v);

/** Why a typed answer cannot be taken, if it cannot. */
function pipedProblem(options: Option[], v: string): string | undefined {
  const picked = findOption(options, v);
  if (!picked) return 'Pick one of the numbers above.';
  if (picked.disabled) return `${picked.label} is ${picked.disabled}. Pick another.`;
}

/** Not a terminal: numbered list, one line of input, validated and re-prompted. */
async function choosePiped(options: Option[], pickable: number[]): Promise<string> {
  renderOptions(options, -1, false);
  const answer = await ask('choice:', String(pickable[0] + 1), { validate: (v) => pipedProblem(options, v) });
  return findOption(options, answer)!.id;
}

/** Draws an interactive menu with the first pickable option highlighted. */
function openMenu(options: Option[], pickable: number[]): Menu {
  const rows = renderOptions(options, pickable[0], true) + 1; // + the hint line
  // Redrawing relies on the whole menu still being on screen; once it has
  // scrolled, moving the cursor up lands somewhere else entirely.
  const canRedraw = rows < (stdout.rows || DEFAULT_ROWS) - 1;
  return { options, pickable, active: pickable[0], rows, canRedraw };
}

/** Settles the menu: the choice is recorded, a cancel rejects. */
function finish(menu: Menu, outcome: Outcome, resolve: (id: string) => void, reject: (e: Error) => void): void {
  if (outcome.id === null) {
    stdout.write('\n');
    reject(new InputError('Cancelled.'));
  } else {
    recordChoice(menu);
    resolve(outcome.id);
  }
}

/** Stops listening for keys and hands the terminal back to line input. */
function release(onData: (buf: Buffer) => void, wasRaw: boolean): void {
  stdin.off('data', onData);
  if (!wasRaw) stdin.setRawMode(false);
  resumeLines();
}

/** Listens for keys until the menu settles. */
function listen(menu: Menu, wasRaw: boolean, resolve: (id: string) => void, reject: (e: Error) => void): void {
  const onData = (buf: Buffer) => {
    const outcome = actionFor(buf.toString())?.(menu);
    if (!outcome) return;
    release(onData, wasRaw);
    finish(menu, outcome, resolve, reject);
  };
  stdin.on('data', onData);
}

/** Reads raw keys until the menu settles. */
const readKeys = (menu: Menu, wasRaw: boolean) =>
  new Promise<string>((resolve, reject) => listen(menu, wasRaw, resolve, reject));

/** Puts the terminal into raw mode for key presses; returns whether it already was. */
function enterRaw(): boolean {
  pauseLines();
  const wasRaw = !!stdin.isRaw;
  stdin.setRawMode(true);
  stdin.resume();
  return wasRaw;
}

/** On a terminal: arrow keys, number keys and enter, redrawn in place. */
async function chooseInteractive(options: Option[], pickable: number[]): Promise<string> {
  const menu = openMenu(options, pickable);
  const wasRaw = enterRaw();
  stdout.write(`${hint()}\n`);
  try {
    return await readKeys(menu, wasRaw);
  } finally {
    if (!wasRaw && stdin.isTTY && stdin.isRaw) stdin.setRawMode(false);
  }
}

/** Asks the user to pick one option and returns its id. */
export async function choose(question: string, options: Option[]): Promise<string> {
  const pickable = options.map((o, i) => (o.disabled ? -1 : i)).filter((i) => i >= 0);
  if (!pickable.length) throw new Error(`No option available for: ${question}`);
  stdout.write(`  ${style.green('?')} ${style.bold(question)}\n`);
  return stdin.isTTY ? chooseInteractive(options, pickable) : choosePiped(options, pickable);
}
