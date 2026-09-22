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

/** The readline, while one is open. */
let rl: Interface | null = null;
/** While true, a terminal echoes `*` for each typed character. */
let masked = false;
/** Lines that arrived before anyone asked for them. */
const pending: string[] = [];
/** Whoever is waiting for the next line. */
let waiter: ((line: string) => void) | null = null;
/** True once input has closed. */
let ended = false;
/** How many lines input really held. The empty line a closed input stands in for is not one of them. */
let linesRead = 0;

/** A line that came from input: counted, then handed on. */
function readLine(line: string): void {
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
  if (waiter) deliver('');
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
  asAny._writeToOutput = (s: string) => {
    if (masked) asAny.output.write(s.includes('\n') ? '\n' : '*');
    else asAny.output.write(s);
  };
}

/** The process's readline, opened on first use. */
function iface(): Interface {
  if (rl) return rl;
  rl = createInterface({ input: stdin, output: stdout, terminal: !!stdin.isTTY });
  rl.on('line', readLine);
  rl.on('close', close);
  maskEcho(rl);
  return rl;
}

/** The next line of input; empty once input has closed. */
export function nextLine(): Promise<string> {
  const queued = pending.shift();
  if (queued !== undefined) return Promise.resolve(queued);
  if (ended) return Promise.resolve('');
  iface();
  return new Promise((resolve) => {
    waiter = resolve;
  });
}

/** Turns echo masking on or off. */
export function setMasked(on: boolean): void {
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
  rl?.pause();
}

/** Lets readline read again after a menu. */
export function resumeLines(): void {
  rl?.resume();
}

/** Restores the terminal and closes the readline, so the process can exit. */
export function closePrompts(): void {
  if (stdin.isTTY && stdin.isRaw) stdin.setRawMode(false);
  rl?.close();
  rl = null;
}
