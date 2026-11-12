/** Opt-in setup framing; a question owns only its own rows and never changes an accepted answer. */
import { stdin, stdout } from 'node:process';
import { plain, style, up, ESC } from './style.ts';
import { cellWidth, contentColumns, displayText, labelValue, terminalRows, wrapProse } from './layout.ts';
import { DEFAULT_COLUMNS, DEFAULT_ROWS, CHOICE_INSET } from './constants.ts';
import type { Option } from './menu.ts';

/** Help is explicit data, including how to obtain an external value. */
export interface QuestionHelp {
  /** Concise title used for the completed answer. */
  title: string;
  /** Optional longer active question, while completed answers retain the concise title. */
  question?: string;
  /** Why the value is needed. */
  purpose: string;
  /** Where to find it, or how to choose it. */
  find: string;
  /** A literal command to display, never execute. */
  command?: string;
  /** A representative value, with no real credentials. */
  example?: string;
  /** Meaning of Enter or a blank value. */
  behavior: string;
}

/** The input engine reports display events without giving secret text to the renderer. */
export interface QuestionPresentation {
  /** Begin a new active block. */
  begin: (label: string, fallback: string, secret: boolean) => void;
  /** Draw the input marker. */
  prompt: () => string;
  /** Account for echoed input using only its cell count. */
  input: (cells: number) => void;
  /** Explain a rejected value without repeating the whole help block. */
  problem: (message: string) => void;
  /** Replace the active block with a safe completed answer. */
  complete: (value: string) => void;
  /** Leave a clear cancellation state. */
  cancel: () => void;
  /** Render readable two-line options. */
  option: (value: Option, index: number, active: boolean, interactive: boolean) => string[];
  /** Whether keyboard navigation and cursor movement are safe. */
  canNavigate: (options: Option[]) => boolean;
  /** Include the menu's final visible region in owned row accounting. */
  menuRows: (rows: number) => void;
  /** Force numbered, append-only rendering for plain or constrained terminals. */
  appendOnly: () => void;
}

/** A setup question and the terminal geometry captured when it began. */
class QuestionFrame implements QuestionPresentation {
  /** Immutable help associated with this field. */
  help: QuestionHelp;
  /** Printed rows which belong to this question only. */
  rows = 0;
  /** Resize disables rewinding to prevent touching older transcript content. */
  columns = stdout.columns || DEFAULT_COLUMNS;
  /** A shorter window must not rewind into an earlier screen region. */
  height = stdout.rows || DEFAULT_ROWS;
  /** Secret input contributes mask cells and is completed as Provided. */
  secret = false;
  /** A matching stream and cursor-capable terminal are required. */
  interactive = !!stdin.isTTY && !!stdout.isTTY && !plain;
  constructor(help: QuestionHelp) {
    this.help = help;
  }
  /** Essential help uses normal foreground; only the rail is subdued. */
  begin(_label: string, _fallback: string, secret: boolean): void {
    this.secret = secret;
    this.line(`\n  ${style.cyan('◆')} ${style.bold(this.help.question || this.help.title)}`);
    for (const text of helpParagraphs(this.help)) this.prose(text);
    if (this.help.command) this.line(`  ${style.grey('│')}   ${displayText(this.help.command)}`);
    this.prose(this.help.behavior);
  }
  /** Text input always begins below its guidance. */
  prompt(): string {
    return `  ${style.cyan('└')} › `;
  }
  /** Readline has already echoed input and advanced to the following row. */
  input(cells: number): void {
    this.rows += Math.max(1, Math.ceil((cellWidth(this.prompt()) + cells) / this.columns));
    if (this.secret) this.rows += 1;
  }
  /** A retry retains every earlier answer and only adds the active field's error. */
  problem(message: string): void {
    this.prose(`Check: ${message}`, true);
  }
  /** Secrets are not passed in; they can only display a fixed status. */
  complete(value: string): void {
    this.rewind();
    setupAnswer(this.help.title, this.secret ? 'Provided' : value);
  }
  /** Cancellation does not pretend a question was answered. */
  cancel(): void {
    this.rewind();
    stdout.write(`  ${style.yellow('!')} Setup cancelled.\n`);
  }
  /** Menu rows are replaced on redraw and counted only once at completion. */
  menuRows(rows: number): void {
    this.rows += rows;
  }
  /** Once fallback occurs, this question never resumes cursor navigation. */
  appendOnly(): void {
    this.interactive = false;
  }
  /** Choice descriptions wrap separately from labels instead of competing on one long line. */
  option(value: Option, index: number, active: boolean, interactive: boolean): string[] {
    const columns = Math.max(1, contentColumns() - CHOICE_INSET);
    const mark = interactive ? (active ? style.cyan('●') : '○') : `${index + 1}.`;
    const first = wrapProse(value.label, columns).map(
      (text, row) => `  ${style.grey('│')} ${row ? '  ' : `${mark} `}${active ? style.bold(text) : text}`,
    );
    return [...first, ...choiceDescription(value.disabled || value.note, columns)];
  }
  /** A tall menu or resized terminal falls back to visible numbered input. */
  canNavigate(options: Option[]): boolean {
    const rows = options
      .flatMap((value, index) => this.option(value, index, false, true))
      .reduce((sum, line) => sum + terminalRows(line), 1);
    return this.canRewind() && this.rows + rows < (stdout.rows || DEFAULT_ROWS) - 1;
  }
  /** Each printed line is accounted for, including native wrapping. */
  line(value: string): void {
    stdout.write(`${value}\n`);
    this.rows += value.split('\n').reduce((sum, line) => sum + terminalRows(line), 0);
  }
  /** Wrap prose to the available rail width. */
  prose(value: string, error = false): void {
    for (const line of wrapProse(value)) this.line(`  ${style.grey('│')} ${error ? style.red(line) : line}`);
  }
  /** Rewind only when the complete owned block remains visible in the same geometry. */
  canRewind(): boolean {
    return (
      this.interactive &&
      this.columns === (stdout.columns || DEFAULT_COLUMNS) &&
      this.height === (stdout.rows || DEFAULT_ROWS) &&
      this.rows < this.height - 1
    );
  }
  /** Erase only the current question, never scrollback or previous answers. */
  rewind(): void {
    if (this.canRewind()) stdout.write(`${up(this.rows)}${ESC}[J`);
  }
}

/** Descriptions sit below their choice label and stay inside the same content width. */
function choiceDescription(value: string | undefined, columns: number): string[] {
  return !value ? [] : wrapProse(value, columns).map((text) => `  ${style.grey('│')}   ${style.grey(text)}`);
}

/** A compact completed value is shared by answered questions, explicit flags and saved plans. */
export function setupAnswer(label: string, value: string): void {
  labelValue(label, value).forEach((line, index) =>
    stdout.write(`${index ? '    ' : `  ${style.green('✓')} `}${line}\n`),
  );
}

/** Help paragraphs are readable in the terminal and keep commands on their own copyable line. */
function helpParagraphs(help: QuestionHelp): string[] {
  return [help.purpose, help.example ? `Example: ${help.example}` : '', `Find: ${help.find}`].filter(Boolean);
}

/** Create a fresh owner for each question; presentation state is never global. */
export const presentQuestion = (help: QuestionHelp): QuestionPresentation => new QuestionFrame(help);

/** A compact phase indicator replaces misleading counts of conditional questions. */
export function setupPhase(phase: string): void {
  const phases = ['Target', 'Configure', 'Deploy', 'Verify'];
  stdout.write(
    `\n  ${phases.map((name) => (name === phase ? style.cyan(style.bold(`[${name}]`)) : style.grey(name))).join(' › ')}\n`,
  );
}

/** Keep the setup identity compact so the first question is visible on a small terminal. */
export function setupIntro(): void {
  stdout.write(`\n  ${style.cyan('◇')} ${style.bold('Oya Browser')}\n  Self-host setup\n`);
  setupPhase('Target');
}

/** Grouped report rows share the same responsive layout as completed answers. */
export function setupSection(title: string, values: Array<[string, string]>): void {
  stdout.write(`\n  ${style.bold(title)}\n`);
  for (const [label, value] of values) for (const line of labelValue(label, value)) stdout.write(`  ${line}\n`);
}
