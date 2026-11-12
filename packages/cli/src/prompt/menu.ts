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
import type { QuestionPresentation as Presentation } from './presentation.ts';

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
  /** Only setup questions opt into the detailed presentation. */
  presentation?: Presentation;
}

/** What a key press does to an open menu; settles the menu by returning an outcome. */
type KeyAction = (menu: Menu) => Outcome | void;
/** How a menu ended: an option's id, or null for a cancel (Ctrl+C). */
interface Outcome {
  /** The chosen option's id, or null when cancelled. */
  id: string | null;
}

/** Menu outcomes settle the one input promise. */
type ResolveChoice = (id: string) => void;
/** Cancellation or input failures reject that same promise. */
type RejectChoice = (error: Error) => void;
/** The raw-key listener is removed before falling back to line input. */
type KeyListener = (buffer: Buffer) => void;

/** The hint under an interactive menu. */
const hint = (presentation?: Presentation) =>
  style.grey(presentation ? '  └ ↑↓ move · Enter · Ctrl+C cancel' : '  ↑↓ move · enter select');

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
function renderOptions(options: Option[], active: number, interactive: boolean, presentation?: Presentation): number {
  return options.reduce((rows, option, index) => {
    const lines = presentation?.option(option, index, interactive && index === active, interactive);
    return rows + writeOptions(lines ?? [optionLine(option, index, interactive && index === active, interactive)]);
  }, 0);
}

/** Measure physical option rows once, whether a legacy line or detailed block is used. */
function writeOptions(lines: string[]): number {
  const columns = stdout.columns || DEFAULT_COLUMNS;
  return lines.reduce((rows, line) => {
    stdout.write(`${clearLine}${line}\n`);
    return rows + Math.max(1, Math.ceil(visible(line) / columns));
  }, 0);
}

/** Draws the menu again over itself, if it is still on screen. */
function redraw(menu: Menu): void {
  if (!menu.canRedraw) return;
  stdout.write(up(menu.rows));
  menu.rows = renderOptions(menu.options, menu.active, true, menu.presentation) + 1;
  stdout.write(`${hint(menu.presentation)}\n`);
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
  if (menu.presentation) return recordPresented(menu);
  const chosen = `  ${icon.arrow} ${style.green(menu.options[menu.active].label)}`;
  if (menu.canRedraw) collapse(menu, chosen);
  else stdout.write(`${chosen}\n`);
}

/** The question owner collapses help and choices together. */
function recordPresented(menu: Menu): void {
  menu.presentation!.menuRows(menu.rows);
  menu.presentation!.complete(menu.options[menu.active].label);
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
async function choosePiped(options: Option[], pickable: number[], presentation?: Presentation): Promise<string> {
  presentation?.appendOnly();
  renderOptions(options, -1, false, presentation);
  const answer = await pipedAnswer(options, pickable[0], !!presentation);
  const picked = findOption(options, answer)!;
  presentation?.complete(picked.label);
  return picked.id;
}

/** Numbered setup selection requires an actual answer, including an explicit blank for the default. */
function pipedAnswer(options: Option[], first: number, requireInput: boolean): Promise<string> {
  return ask('choice:', String(first + 1), { validate: (value) => pipedProblem(options, value), requireInput });
}

/** Draws an interactive menu with the first pickable option highlighted. */
function openMenu(options: Option[], pickable: number[], presentation?: Presentation): Menu {
  const rows = renderOptions(options, pickable[0], true, presentation) + 1; // + the hint line
  // Redrawing relies on the whole menu still being on screen; once it has
  // scrolled, moving the cursor up lands somewhere else entirely.
  const canRedraw = rows < (stdout.rows || DEFAULT_ROWS) - 1;
  return { options, pickable, active: pickable[0], rows, canRedraw, presentation };
}

/** Settles the menu: the choice is recorded, a cancel rejects. */
function finish(menu: Menu, outcome: Outcome, resolve: (id: string) => void, reject: (e: Error) => void): void {
  if (outcome.id === null) return cancelMenu(menu, reject);
  recordChoice(menu);
  resolve(outcome.id);
}

/** Leave a clear cancellation state without recording an answer. */
function cancelMenu(menu: Menu, reject: RejectChoice): void {
  menu.presentation?.menuRows(menu.rows);
  menu.presentation?.cancel();
  stdout.write('\n');
  reject(new InputError('Cancelled.'));
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
    if (windowChanged(menu, buf)) return resizeFallback(menu, onData, wasRaw, resolve, reject);
    const outcome = actionFor(buf.toString())?.(menu);
    if (!outcome) return;
    release(onData, wasRaw);
    finish(menu, outcome, resolve, reject);
  };
  stdin.on('data', onData);
}

/** Detailed menus own their geometry and must stop cursor navigation after a resize. */
function windowChanged(menu: Menu, buffer: Buffer): boolean {
  return buffer.toString() !== '\u0003' && !!menu.presentation && !menu.presentation.canNavigate(menu.options);
}

/** Stop raw navigation after resize and offer a visible numbered choice instead. */
function resizeFallback(
  menu: Menu,
  onData: KeyListener,
  wasRaw: boolean,
  resolve: ResolveChoice,
  reject: RejectChoice,
): void {
  release(onData, wasRaw);
  choosePiped(menu.options, menu.pickable, menu.presentation).then(resolve, reject);
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
async function chooseInteractive(options: Option[], pickable: number[], presentation?: Presentation): Promise<string> {
  const menu = openMenu(options, pickable, presentation);
  const wasRaw = enterRaw();
  stdout.write(`${hint(presentation)}\n`);
  try {
    return await readKeys(menu, wasRaw);
  } finally {
    if (!wasRaw && stdin.isTTY && stdin.isRaw) stdin.setRawMode(false);
  }
}

/** Asks the user to pick one option and returns its id. */
export async function choose(question: string, options: Option[], presentation?: Presentation): Promise<string> {
  const pickable = options.map((o, i) => (o.disabled ? -1 : i)).filter((i) => i >= 0);
  if (!pickable.length) throw new Error(`No option available for: ${question}`);
  if (presentation) presentation.begin(question, '', false);
  else stdout.write(`  ${style.green('?')} ${style.bold(question)}\n`);
  return chooseInput(options, pickable, presentation);
}

/** Plain or constrained setup terminals use numbered input instead of invisible raw-key navigation. */
function chooseInput(options: Option[], pickable: number[], presentation?: Presentation): Promise<string> {
  const interactive = presentation ? presentation.canNavigate(options) : stdin.isTTY;
  return interactive
    ? chooseInteractive(options, pickable, presentation)
    : choosePiped(options, pickable, presentation);
}
