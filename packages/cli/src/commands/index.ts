/**
 * Every `oya` command by name: the command map the entry point dispatches
 * through. Control-plane commands are routed first (see control.ts).
 */
import type { Flags } from '../args.ts';
import { cmdInstall } from '../install.ts';
import { cmdLogin } from './login.ts';
import { cmdInit } from './init.ts';
import { cmdAsk, cmdGoto, cmdLs, cmdOpen, cmdRm, cmdStart, cmdStatus } from './browsers.ts';
import { cmdPersonas } from './personas.ts';
import { cmdCookies } from './cookies.ts';
import { cmdConfig, cmdUsage, cmdWhoami } from './settings.ts';
import { cmdStealthTest } from './stealth.ts';
import { isControlCommand, runControl } from './control.ts';
import { HELP } from '../help.ts';
import { usage } from '../errors.ts';
import { out } from '../context.ts';
import { createRequire } from 'node:module';

/** A command: its positional arguments and flags. */
type Command = (args: string[], flags: Flags) => Promise<void>;

/** Prints the help text. */
const help: Command = async () => console.log(HELP);

/**
 * This CLI's version, from its package.json: one folder up from the built
 * dist/index.js, two from this source file (which the tests run).
 */
function packageVersion(): string {
  const require = createRequire(import.meta.url);
  for (const path of ['../package.json', '../../package.json']) {
    try {
      return require(path).version;
    } catch {}
  }
  return 'unknown';
}

/** This CLI's version. */
const VERSION = packageVersion();

/** Prints the version: `oya 1.0.110`, or `{"version":"1.0.110"}` with --json. */
const version: Command = async (_args, flags) => out(flags, { version: VERSION }, () => console.log(`oya ${VERSION}`));

/**
 * A command whose output is a person's (a wizard, a detector's report), which
 * --json would only cut up: refused before it starts, saying what to use.
 */
const noJson =
  (command: string, run: Command, instead: string): Command =>
  async (args, flags) => {
    if (flags.json) throw usage(`oya ${command} has no --json output. ${instead}`);
    await run(args, flags);
  };

/** The commands, by name; aliases share a handler. */
export const COMMANDS: Record<string, Command> = {
  install: noJson('install', (_args, flags) => cmdInstall(flags), 'For an unattended install, use --config <file>.'),
  login: (_args, flags) => cmdLogin(flags),
  init: noJson('init', (_args, flags) => cmdInit(flags), 'Use oya config key=value --json.'),
  start: (_args, flags) => cmdStart(flags),
  goto: cmdGoto,
  ask: cmdAsk,
  ls: (_args, flags) => cmdLs(flags),
  list: (_args, flags) => cmdLs(flags),
  status: (_args, flags) => cmdStatus(flags),
  rm: cmdRm,
  stop: cmdRm,
  personas: cmdPersonas,
  cookies: cmdCookies,
  open: (_args, flags) => cmdOpen(flags),
  config: cmdConfig,
  usage: (_args, flags) => cmdUsage(flags),
  'stealth-test': noJson('stealth-test', (_args, flags) => cmdStealthTest(flags), 'Read its report from the terminal.'),
  whoami: (_args, flags) => cmdWhoami(flags),
  version,
  help,
  '--help': help,
  '-h': help,
};

/** An unknown command: nothing is run, and the help is one command away. */
const unknown = (command: string) => usage(`Unknown command "${command}". Run oya help for the list.`);

/** Whether help was asked for anywhere on the line: `--help` as a flag, or `-h`, which parses as a word. */
const wantsHelp = (args: string[], flags: Flags) => Object.hasOwn(flags, 'help') || args.includes('-h');

/**
 * Runs one command. A line that asks for help gets the help and nothing else:
 * `oya rm --all --help` must never stop a fleet, nor `oya install --help` install.
 */
export async function dispatch(command: string, args: string[], flags: Flags): Promise<void> {
  if (wantsHelp(args, flags)) return help(args, flags);
  if (flags.version || command === '--version') return version(args, flags);
  if (isControlCommand(command, flags)) return runControl(command, args, flags);
  if (!Object.hasOwn(COMMANDS, command)) throw unknown(command);
  await COMMANDS[command](args, flags);
}
