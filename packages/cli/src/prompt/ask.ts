/**
 * Questions answered with a line of text: plain, secret, and yes/no. Bad
 * answers are re-asked, never fatal, while there is input left to read.
 */
import { stdin, stdout } from 'node:process';
import { icon, style } from './style.ts';
import { inputExhausted, nextLine, setMasked } from './lines.ts';

/** Thrown when input is rejected and there is no interactive way to retry. */
export class InputError extends Error {}

/** How an answer is read and checked. */
export interface AskOptions {
  /** Return a message to reject and re-prompt; return nothing to accept. */
  validate?: (value: string) => string | void;
  /** Mask what is typed. */
  secret?: boolean;
}

/** Ends masking, moving past the line the hidden answer was typed on. */
function unmask(): void {
  setMasked(false);
  stdout.write('\n');
}

/** One line, trimmed, or the fallback when blank. Masking ends however the read ends. */
async function readAnswer(fallback: string, secret: boolean): Promise<string> {
  setMasked(secret);
  try {
    return (await nextLine()).trim() || fallback;
  } finally {
    if (secret) unmask();
  }
}

/** The answer if it passes validation; otherwise says why, and throws when nothing more can be read. */
function check(answer: string, options: AskOptions): string | undefined {
  // Off a terminal the answer is not echoed, so echo it to keep the transcript readable.
  if (!stdin.isTTY && !options.secret) stdout.write(`${answer}\n`);
  const problem = options.validate?.(answer);
  if (!problem) return answer;
  stdout.write(`  ${icon.fail} ${style.red(problem)}\n`);
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
    const accepted = check(await readAnswer(fallback, !!options.secret), options);
    if (accepted !== undefined) return accepted;
  }
}

/** Asks a question; a blank answer takes `fallback`, shown in brackets. */
export function ask(question: string, fallback = '', options: AskOptions = {}): Promise<string> {
  return readValidated(
    () => stdout.write(`  ${style.green('?')} ${question}${fallback ? style.grey(` (${fallback})`) : ''} `),
    fallback,
    options,
  );
}

/** Same, but the terminal does not echo, for keys and secrets. */
export function askSecret(question: string, options: AskOptions = {}): Promise<string> {
  return readValidated(() => stdout.write(`  ${style.green('?')} ${question} `), '', { ...options, secret: true });
}

/** A yes/no question; anything starting with y is yes. */
export async function confirm(question: string, fallback = false): Promise<boolean> {
  const answer = await ask(`${question} ${style.grey(fallback ? '[Y/n]' : '[y/N]')}`, fallback ? 'y' : 'n');
  return /^y/i.test(answer);
}
