/**
 * Line input for the whole process.
 *
 * One readline for the whole process, and our own line queue rather than
 * rl.question(): closing a readline discards what it has buffered, and readline
 * emits 'line' for every buffered line at once while question() only catches the
 * one arriving as it waits. Queueing them is what makes piped input work at all.
 */
import { createInterface, type Interface } from 'node:readline';
import { stdin, stdout } from 'node:process';
import { plain, ESC } from './style.ts';
import { PlainInput } from './plain-input.ts';
import { readlineOutput } from './readline-output.ts';

/** The readline, while one is open. */
let rl: Interface | null = null;
/** While true, a terminal echoes `*` for each typed character. */
let masked = false;
/** Only mask cells, never secret content, are reported to setup layout. */
let maskCells = 0;
/** Lines that arrived before anyone asked for them. */
const pending: string[] = [];
/** Whoever is waiting for the next line. */
let waiter: ((line: string) => void) | null = null;
/** True once input has closed. */
let ended = false;
/** How many lines input really held. The empty line a closed input stands in for is not one of them. */
let linesRead = 0;
/** Distinguish terminal cancellation from an intentionally submitted blank line. */
let interrupted = false;
/** EOF supplies no answer and must not accept setup defaults implicitly. */
let eofLine = false;
/** Readline needs the visible prefix to edit wrapped input without erasing the rail. */
let currentPrompt = '';
/** Only opt-in setup questions filter Readline refresh controls in plain mode. */
let setupPlainInput = false;
/** A setup reader must not retain credentials in interactive history. */
let setupInput = false;
/** One echo collaborator belongs to the one input reader. */
let plainInput: PlainInput | null = null;
/** Only listeners created by our reader are suspended during a raw menu. */
type ReaderListener = (...args: unknown[]) => void;
/** The reader's key parser must not consume menu navigation. */
let readerKeys: ReaderListener[] = [];
/** Readline may not redraw over a raw menu on terminal resize. */
let readerResize: ReaderListener[] = [];

/** A line that came from input: counted, then handed on. */
function readLine(line: string): void {
  eofLine = false;
  linesRead++;
  deliver(line);
}

/** Hands a line to whoever waits for it, or queues it. */
function deliver(line: string): void {
  if (waiter) {
    const w = waiter;
    waiter = null;
    w(line);
  } else pending.push(line);
}

/** Input closed: a waiting reader gets an empty line. */
function close(): void {
  ended = true;
  if (waiter) {
    eofLine = true;
    deliver('');
  }
}

/** Ctrl+C cancels the pending question instead of accepting every remaining default. */
function interruptLine(): void {
  interrupted = true;
  stdout.write('\n');
  rl?.close();
}

/** The readline internals masking overrides. */
interface EchoingInterface {
  /** Where readline echoes to. */
  output: NodeJS.WriteStream;
  /** readline's echo hook. */
  _writeToOutput?: (s: string) => void;
}

/** Only a terminal echoes what is typed, so masking only has to work there. */
function maskEcho(line: Interface): void {
  const asAny = line as unknown as EchoingInterface;
  const output = asAny.output;
  const display = new PlainInput(line, output, () => currentPrompt);
  plainInput = display;
  connectEcho(asAny, output, display);
}

/** Text and direct cursor writes share one active display policy. */
function connectEcho(echo: EchoingInterface, output: NodeJS.WriteStream, display: PlainInput): void {
  echo.output = readlineOutput(output, (text) => plainWrite(display, output, text));
  echo._writeToOutput = (s: string) => {
    if (masked) writeMask(output, s.includes('\n'));
    else if (setupPlainInput) display.write(s);
    else output.write(s);
  };
}

/** Direct cursor operations must pass the same plain/masked boundary as text refreshes. */
function plainWrite(display: PlainInput, output: NodeJS.WriteStream, text: string): boolean {
  if (!setupPlainInput) return false;
  if (!masked) display.write(text);
  else if (!text.includes(ESC)) writeMask(output, text.includes('\n'));
  return true;
}

/** Count the exact mask glyphs emitted, including pasted values. */
function writeMask(output: NodeJS.WriteStream, newline: boolean): void {
  if (!newline) maskCells++;
  output.write(newline ? '\n' : '*');
}

/** Return display geometry without revealing the masked line. */
export const maskedCells = (): number => maskCells;

/** The process's readline, opened on first use. */
function iface(): Interface {
  if (rl) return rl;
  rl = createReader();
  rl.setPrompt(currentPrompt);
  rl.on('line', readLine);
  rl.on('close', close);
  rl.on('SIGINT', interruptLine);
  maskEcho(rl);
  return rl;
}

/** Capture the listeners introduced by this reader, preserving all unrelated stream listeners. */
function createReader(): Interface {
  const keys = new Set<unknown>(stdin.listeners('keypress')),
    resize = new Set<unknown>(stdout.listeners('resize'));
  const line = openReader();
  readerKeys = addedListeners(keys, stdin.listeners('keypress'));
  readerResize = addedListeners(resize, stdout.listeners('resize'));
  return line;
}

/** Only the opt-in setup reader disables history; legacy prompt defaults are preserved. */
function openReader(): Interface {
  return createInterface({
    input: stdin,
    output: stdout,
    terminal: !!stdin.isTTY,
    historySize: setupInput ? 0 : undefined,
  });
}

/** Ownership is identified at construction through public EventEmitter APIs. */
function addedListeners(before: Set<unknown>, after: unknown[]): ReaderListener[] {
  return after.filter((listener) => !before.has(listener)) as ReaderListener[];
}

/** Share the displayed marker with the existing input reader; this does not draw or read another line. */
export function setLinePrompt(value: string, setup = false): void {
  currentPrompt = value;
  setupInput = setup;
  setupPlainInput = setup && plain;
  plainInput?.reset();
  rl?.setPrompt(value);
}

/** The next line of input; empty once input has closed. */
export function nextLine(): Promise<string> {
  const queued = pending.shift();
  if (queued !== undefined) return resolvedLine(queued, false);
  if (ended) return resolvedLine('', true);
  iface();
  return new Promise((resolve) => {
    waiter = resolve;
  });
}

/** Queue reads distinguish a real blank submission from the synthetic EOF placeholder. */
function resolvedLine(value: string, eof: boolean): Promise<string> {
  eofLine = eof;
  return Promise.resolve(value);
}

/** Cancellation status is separate from EOF so errors can explain what happened. */
export const inputInterrupted = (): boolean => interrupted;
/** Setup rejects missing input while preserving an actual submitted blank line. */
export const lineWasEOF = (): boolean => eofLine;

/** Turns echo masking on or off. */
export function setMasked(on: boolean): void {
  if (on) maskCells = 0;
  masked = on;
}

/**
 * Whether anyone answered anything. Piped blank lines are answers: each accepts a
 * default on purpose. Off a terminal with no line read, nobody answered at all,
 * and every default a wizard then "accepted" was never agreed to.
 */
export function answered(): boolean {
  return !!stdin.isTTY || linesRead > 0;
}

/** True when input has closed and nothing is left to read, so a re-prompt could never be answered. */
export function inputExhausted(): boolean {
  return ended && !pending.length;
}

/** Stops readline from reading while a menu takes raw keys. */
export function pauseLines(): void {
  for (const listener of readerKeys) stdin.off('keypress', listener);
  for (const listener of readerResize) stdout.off('resize', listener);
  if (rl && !ended) rl.pause();
}

/** Lets readline read again after a menu. */
export function resumeLines(): void {
  if (!rl || ended) return;
  for (const listener of readerKeys)
    if (!stdin.listeners('keypress').includes(listener)) stdin.on('keypress', listener);
  for (const listener of readerResize)
    if (!stdout.listeners('resize').includes(listener)) stdout.on('resize', listener);
  rl.resume();
}

/** Restores the terminal and closes the readline, so the process can exit. */
export function closePrompts(): void {
  if (stdin.isTTY && stdin.isRaw) stdin.setRawMode(false);
  rl?.close();
  rl = null;
  plainInput = null;
  readerKeys = [];
  readerResize = [];
  stdin.pause();
}
