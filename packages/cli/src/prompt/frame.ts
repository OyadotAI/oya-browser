/**
 * Framing around the questions: banners, numbered steps, notes and spinners.
 */
import { stdout } from 'node:process';
import { clearLine, icon, plain, style, width } from './style.ts';
import { SPINNER_FRAME_MS } from './constants.ts';

/** What a spinner is stopped with. */
export interface Spinner {
  /** Stops with a tick and an optional message. */
  done: (msg?: string) => unknown;
  /** Stops with a cross and an optional message. */
  fail: (msg?: string) => unknown;
}

/** The spinner's animation frames. */
const FRAMES = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/** Position in the numbered steps: which one is showing, out of how many. */
const counter = { number: 0, total: 0 };

/** A title between two rules, with an optional subtitle. */
export function banner(title: string, subtitle?: string): void {
  const line = '─'.repeat(width());
  stdout.write(`\n${style.grey(line)}\n  ${style.bold(title)}\n`);
  if (subtitle) stdout.write(`  ${style.grey(subtitle)}\n`);
  stdout.write(`${style.grey(line)}\n`);
}

/** Starts numbering steps out of `total`; zero shows no counter. */
export function steps(total: number): void {
  counter.total = total;
  counter.number = 0;
}

/** The next step's heading. */
export function step(title: string, hint?: string): void {
  counter.number += 1;
  const shown = counter.total ? style.grey(`${counter.number}/${counter.total}`) : '';
  stdout.write(`\n${style.cyan('◆')} ${style.bold(title)} ${shown}\n`);
  if (hint) stdout.write(`${style.grey('  ' + hint)}\n`);
}

/** A dimmed aside. */
export function note(text: string): void {
  stdout.write(`${style.grey('  ' + text)}\n`);
}

/** A ticked line. */
export function success(text: string): void {
  stdout.write(`${icon.ok} ${text}\n`);
}

/** A warning line. */
export function warn(text: string): void {
  stdout.write(`${icon.warn} ${style.yellow(text)}\n`);
}

/** Animates a spinner until stopped. */
function animate(label: string): () => void {
  let i = 0;
  const timer = setInterval(() => {
    stdout.write(`\r  ${style.cyan(FRAMES[i++ % FRAMES.length])} ${label}…   `);
  }, SPINNER_FRAME_MS);
  return () => clearInterval(timer);
}

/** A spinner for work slow enough that silence reads as a hang. */
export function spinner(label: string): Spinner {
  if (plain) return plainSpinner(label);
  const halt = animate(label);
  const stop = (mark: string, msg: string) => {
    halt();
    stdout.write(`\r  ${mark} ${label}  ${style.grey(msg)}${clearLine}\n`);
  };
  return { done: (msg = '') => stop(icon.ok, msg), fail: (msg = '') => stop(icon.fail, msg) };
}

/** Off a terminal: the label, then the outcome on the same line. */
function plainSpinner(label: string): Spinner {
  stdout.write(`  ${label}… `);
  return {
    done: (msg = 'ok') => stdout.write(`${msg}\n`),
    fail: (msg = 'failed') => stdout.write(`${msg}\n`),
  };
}
