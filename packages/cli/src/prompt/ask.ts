/**
 * Questions answered with a line of text: plain, secret, and yes/no. Bad
 * answers are re-asked, never fatal, while there is input left to read.
 */
import { stdin, stdout } from 'node:process';
import { icon, style } from './style.ts';
import {
  inputExhausted,
  nextLine,
  setMasked,
  maskedCells,
  inputInterrupted,
  lineWasEOF,
  setLinePrompt,
} from './lines.ts';
import { cellWidth } from './layout.ts';
import type { QuestionPresentation } from './presentation.ts';

/** Thrown when input is rejected and there is no interactive way to retry. */
export class InputError extends Error {}

/** How an answer is read and checked. */
export interface AskOptions {
  /** Return a message to reject and re-prompt; return nothing to accept. */
  validate?: (value: string) => string | void;
  /** Mask what is typed. */
  secret?: boolean;
  /** Optional setup framing, without changing accepted values. */
  presentation?: QuestionPresentation;
  /** A setup default still needs Enter; EOF is not consent to deploy. */
  requireInput?: boolean;
}

/** Ends masking, moving past the line the hidden answer was typed on. */
function unmask(): void {
  setMasked(false);
  stdout.write('\n');
}

/** One line, trimmed, or the fallback when blank. Masking ends however the read ends. */
async function readAnswer(fallback: string, options: AskOptions): Promise<string> {
  setMasked(!!options.secret);
  try {
    const raw = await nextLine();
    options.presentation?.input(options.secret ? maskedCells() : cellWidth(raw));
    return acceptedLine(raw, fallback, options);
  } finally {
    if (options.secret) unmask();
  }
}

/** Empty input is a valid explicit default, while cancellation and setup EOF are terminal outcomes. */
function acceptedLine(raw: string, fallback: string, options: AskOptions): string {
  if (inputInterrupted()) throw new InputError('Cancelled.');
  if ((options.requireInput || options.presentation) && lineWasEOF())
    throw new InputError('Input ended before setup was complete. Use --yes or --config for unattended installs.');
  return raw.trim() || fallback;
}

/** The answer if it passes validation; otherwise says why, and throws when nothing more can be read. */
function check(answer: string, options: AskOptions): string | undefined {
  // Off a terminal the answer is not echoed, so echo it to keep the transcript readable.
  if (!stdin.isTTY && !options.secret) stdout.write(`${answer}\n`);
  const problem = options.validate?.(answer);
  if (!problem) return answer;
  if (options.presentation) options.presentation.problem(problem);
  else stdout.write(`  ${icon.fail} ${style.red(problem)}\n`);
  // Re-prompting needs somewhere to read from; a closed pipe has nothing left.
  if (inputExhausted()) throw new InputError(problem);
  return undefined;
}

/**
 * Re-prompts rather than throwing. A wizard that exits on a typo throws away
 * every answer given so far, which is the worst possible response to a typo.
 */
async function readValidated(render: () => void, fallback: string, options: AskOptions): Promise<string> {
  for (;;) {
    render();
    const accepted = check(await readAnswer(fallback, options), options);
    if (accepted !== undefined) {
      options.presentation?.complete(options.secret ? 'Provided' : accepted);
      return accepted;
    }
  }
}

/** Asks a question; a blank answer takes `fallback`, shown in brackets. */
export function ask(question: string, fallback = '', options: AskOptions = {}): Promise<string> {
  options.presentation?.begin(question, fallback, !!options.secret);
  return questionResult(() => renderPrompt(question, fallback, options), fallback, options);
}

/** Cancellation redraws only a presented active question; ordinary errors retain their original explanation. */
function questionResult(render: () => void, fallback: string, options: AskOptions): Promise<string> {
  return readValidated(render, fallback, options).catch((err) => {
    if (inputInterrupted()) options.presentation?.cancel();
    throw err;
  });
}

/** Setup framing and legacy text share the same input/validation loop. */
function promptText(question: string, fallback: string, options: AskOptions): string {
  return (
    options.presentation?.prompt() ??
    `  ${style.green('?')} ${question}${fallback ? style.grey(` (${fallback})`) : ''} `
  );
}

/** Keep Readline's cursor geometry aligned with the actual visible input prefix. */
function renderPrompt(question: string, fallback: string, options: AskOptions): void {
  const text = promptText(question, fallback, options);
  setLinePrompt(text, !!options.presentation || !!options.requireInput);
  stdout.write(text);
}

/** Same, but the terminal does not echo, for keys and secrets. */
export function askSecret(question: string, options: AskOptions = {}): Promise<string> {
  return ask(question, '', { ...options, secret: true });
}

/** A yes/no question; anything starting with y is yes. */
export async function confirm(question: string, fallback = false): Promise<boolean> {
  const answer = await ask(`${question} ${style.grey(fallback ? '[Y/n]' : '[y/N]')}`, fallback ? 'y' : 'n');
  return /^y/i.test(answer);
}
