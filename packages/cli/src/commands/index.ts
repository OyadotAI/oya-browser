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
import { cmdConfig, cmdUsage, cmdWhoami } from './settings.ts';
import { cmdStealthTest } from './stealth.ts';
import { isControlCommand, runControl } from './control.ts';
import { HELP } from '../help.ts';

/** A command: its positional arguments and flags. */
type Command = (args: string[], flags: Flags) => Promise<void>;

/** Prints the help text. */
const help: Command = async () => console.log(HELP);

/** The commands, by name; aliases share a handler. */
export const COMMANDS: Record<string, Command> = {
  install: (_args, flags) => cmdInstall(flags),
  login: (_args, flags) => cmdLogin(flags),
  init: (_args, flags) => cmdInit(flags),
  start: (_args, flags) => cmdStart(flags),
  goto: cmdGoto,
  ask: cmdAsk,
  ls: (_args, flags) => cmdLs(flags),
  list: (_args, flags) => cmdLs(flags),
  status: (_args, flags) => cmdStatus(flags),
  rm: cmdRm,
  stop: cmdRm,
  personas: cmdPersonas,
  open: (_args, flags) => cmdOpen(flags),
  config: cmdConfig,
  usage: (_args, flags) => cmdUsage(flags),
  'stealth-test': (_args, flags) => cmdStealthTest(flags),
  whoami: () => cmdWhoami(),
  help,
  '--help': help,
  '-h': help,
};

/** An unknown command: say so, show the help, and exit non-zero. */
function unknown(command: string): never {
  console.error(`Unknown command: ${command}\n`);
  console.log(HELP);
  process.exit(1);
}

/** Runs one command. */
export async function dispatch(command: string, args: string[], flags: Flags): Promise<void> {
  if (isControlCommand(command, flags)) return runControl(command, args, flags);
  if (!Object.hasOwn(COMMANDS, command)) unknown(command);
  await COMMANDS[command](args, flags);
}
