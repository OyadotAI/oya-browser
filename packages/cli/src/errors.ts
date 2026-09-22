/**
 * The CLI's own failures: a message, a code a script can branch on, and an
 * optional hint line. A usage error means nothing was sent, and exits 2 so a
 * script can tell "fix the command" from "retry or look into it".
 */

/** How a CliError is shown. */
export interface CliErrorOptions {
  /** A second line with the next step. */
  hint?: string;
  /** True when the command already printed its own lines for it. */
  shown?: boolean;
}

/** A failure the CLI raises itself, with the code it prints under --json. */
export class CliError extends Error {
  /** usage, invalid_json, unreachable, no_api_key, no_browser, partial_failure, and the like. */
  readonly code: string;
  /** A second line with the next step, when there is one. */
  readonly hint?: string;
  /** Already printed by the command (a partial failure's own lines), so not printed again. */
  readonly shown: boolean;

  /** Records the message, code, hint and whether it was shown. */
  constructor(message: string, code: string, { hint, shown = false }: CliErrorOptions = {}) {
    super(message);
    this.name = 'CliError';
    this.code = code;
    this.hint = hint;
    this.shown = shown;
  }
}

/** A command that could not be run as typed: nothing was sent. */
export const usage = (message: string, hint?: string) => new CliError(message, 'usage', { hint });

/** JSON the caller typed or pointed at that does not parse: nothing was sent either. */
export const invalidJson = (message: string, hint?: string) => new CliError(message, 'invalid_json', { hint });
