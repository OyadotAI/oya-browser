/**
 * Command-line parsing: the command, its positional arguments, and `--flags`.
 * A flag followed by a value takes it; a flag followed by another flag, or by
 * nothing, is `true`.
 */

/** Parsed flags: a value, or `true` for a bare flag. */
export type Flags = Record<string, string | boolean>;

/** What `oya` was asked to do. */
export interface Invocation {
  /** The command; `help` when none was given. */
  command: string;
  /** Positional arguments after it. */
  args: string[];
  /** Its flags. */
  flags: Flags;
}

/** Splits argv (without node and the script) into command, arguments and flags. */
export function parse(argv: string[]): Invocation {
  const [command = 'help', ...rest] = argv;
  const args: string[] = [];
  const flags: Flags = {};
  for (let i = 0; i < rest.length; i++) {
    if (!rest[i].startsWith('--')) args.push(rest[i]);
    else if (takeFlag(rest[i].slice('--'.length), rest[i + 1], flags)) i++;
  }
  return { command, args, flags };
}

/** Records one flag; returns true when it consumed the next token as its value. */
function takeFlag(name: string, next: string | undefined, flags: Flags): boolean {
  const bare = next === undefined || next.startsWith('--');
  flags[name] = bare ? true : next;
  return !bare;
}

/** A flag's value, when it was given one. */
export const flagStr = (flags: Flags, name: string): string | undefined =>
  typeof flags[name] === 'string' ? (flags[name] as string) : undefined;

/** A flag's value as a number, when it was given one. */
export const flagNum = (flags: Flags, name: string): number | undefined =>
  flagStr(flags, name) === undefined ? undefined : Number(flagStr(flags, name));
