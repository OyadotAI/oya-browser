/** Translate Readline display refreshes into plain input updates while retaining its parsing and masking. */
import type { Interface } from 'node:readline';
import { cellWidth, displayText } from './layout.ts';
import { DEFAULT_COLUMNS } from './constants.ts';

/** A display snapshot contains only a nonsecret line and its cursor geometry. */
interface InputSnapshot {
  /** Input already visible to the operator. */
  value: string;
  /** Cursor cells from the start of that input. */
  cursor: number;
  /** Total input width. */
  width: number;
}

/** Reset display state without copying a previous question or credential. */
const emptySnapshot = (): InputSnapshot => ({ value: '', cursor: 0, width: 0 });

/** Readline remains the sole reader; this collaborator owns only plain nonsecret echo. */
export class PlainInput {
  /** The existing reader supplies its accepted editing state. */
  line: Interface;
  /** Destination chosen by the existing reader. */
  output: NodeJS.WriteStream;
  /** Current visible prompt, including its rail. */
  prompt: () => string;
  /** Last state actually rendered. */
  previous = emptySnapshot();
  constructor(line: Interface, output: NodeJS.WriteStream, prompt: () => string) {
    this.line = line;
    this.output = output;
    this.prompt = prompt;
  }
  /** Each question starts without retaining the previous answer. */
  reset(): void {
    this.previous = emptySnapshot();
  }
  /** Internal ANSI refresh calls are replaced with the reader's current nonsecret state. */
  write(text: string): void {
    if (text.includes('\n')) return this.newLine();
    const next = this.snapshot();
    if (next.value === this.previous.value && next.cursor === this.previous.cursor) return;
    this.render(next);
    this.previous = next;
  }
  /** A submitted line ends its display snapshot before the next question starts. */
  newLine(): void {
    this.output.write('\n');
    this.reset();
  }
  /** Measure the current line without interpreting control bytes as display instructions. */
  snapshot(): InputSnapshot {
    const value = this.line.line || '',
      cursor = this.line.cursor ?? value.length;
    return { value: displayText(value), cursor: cellWidth(value.slice(0, cursor)), width: cellWidth(value) };
  }
  /** Ordinary typing appends; corrections repaint only a short active input row. */
  render(next: InputSnapshot): void {
    if (this.appending(next)) {
      this.output.write(next.value.slice(this.previous.value.length));
      return;
    }
    const width = cellWidth(this.prompt()) + Math.max(next.width, this.previous.width);
    if (this.output.isTTY && width < (this.output.columns || DEFAULT_COLUMNS)) this.replace(next);
    else this.output.write(`\n${this.prompt()}${next.value}`);
  }
  /** Fast-path appends keep ordinary plain transcripts concise. */
  appending(next: InputSnapshot): boolean {
    return (
      next.value.startsWith(this.previous.value) &&
      this.previous.cursor === this.previous.width &&
      next.cursor === next.width
    );
  }
  /** Basic carriage return/backspace edits stay inside the one input row and emit no ANSI. */
  replace(next: InputSnapshot): void {
    const padding = Math.max(0, this.previous.width - next.width);
    const back = next.width - next.cursor + padding;
    this.output.write(`\r${this.prompt()}${next.value}${' '.repeat(padding)}${'\b'.repeat(back)}`);
  }
}
