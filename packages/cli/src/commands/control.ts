/**
 * The durable control-plane commands (`oya control`, `sessions`, `takeover`,
 * `credential`, `webhook` …). Each prints the API's answer as JSON. They
 * dispatch through a command map, and so do their own subcommands.
 */
import type { ControlRole, Oya } from '@oya-ai/browser';
import { flagStr, type Flags } from '../args.ts';
import { client, printJson } from '../context.ts';

/** The control API. */
type Control = Oya['control'];

/** What a control command is run with. */
interface ControlCall {
  /** The control API for this key. */
  c: Control;
  /** The command name, for commands that share a handler. */
  command: string;
  /** Positional arguments. */
  args: string[];
  /** Flags. */
  flags: Flags;
}

/** A control command: returns what to print. */
type Handler = (call: ControlCall) => Promise<unknown>;

/** Roles a member or credential may be given. */
const ROLES = ['viewer', 'operator', 'administrator'];

/** The argument at `i`, or an error pointing at `oya help`. */
function required(args: string[], i = 0): string {
  if (!args[i]) throw new Error('Missing argument; run oya help');
  return args[i];
}

/** `--role`, defaulting to operator, checked against the roles that exist. */
function roleFlag(flags: Flags): ControlRole {
  const role = flagStr(flags, 'role') || 'operator';
  if (!ROLES.includes(role)) throw new Error('Invalid role');
  return role as ControlRole;
}

/** Runs the subcommand named by `sub`, or throws `usage`. */
function subcommand(table: Record<string, Handler>, sub: string, call: ControlCall, usage: string): Promise<unknown> {
  if (!Object.hasOwn(table, sub)) throw new Error(usage);
  return table[sub](call);
}

/** `oya members [invite|remove <user>]`. */
const members: Handler = (call) => {
  if (!call.args[0]) return call.c.members();
  const table: Record<string, Handler> = {
    remove: ({ c, args }) => c.removeMember(required(args, 1)),
    invite: ({ c, flags }) => c.inviteMember(roleFlag(flags)),
  };
  return subcommand(table, call.args[0], call, 'Use members, members invite, or members remove');
};

/** `oya credential new|revoke`. */
const credential: Handler = (call) => {
  const table: Record<string, Handler> = {
    revoke: ({ c, args }) => c.revokeCredential(required(args, 1)),
    new: ({ c, flags }) => c.createCredential({ role: roleFlag(flags), label: flagStr(flags, 'label') }),
  };
  return subcommand(table, required(call.args), call, 'Use credential new or credential revoke');
};

/** `oya webhook new|remove|replay`. */
const webhook: Handler = (call) => {
  const table: Record<string, Handler> = {
    new: ({ c, args }) => c.createWebhook(required(args, 1)),
    remove: ({ c, args }) => c.removeWebhook(required(args, 1)),
    replay: ({ c, args }) => c.replayDelivery(required(args, 1)),
  };
  return subcommand(table, required(call.args), call, 'Use webhook new, remove, or replay');
};

/** `takeover` acquires; `release` and `resume` are sent as named. */
const takeover: Handler = ({ c, command, args }) =>
  c.takeover(required(args), command === 'takeover' ? 'acquire' : (command as 'release' | 'resume'));

/** Every control command by name. */
const COMMANDS: Record<string, Handler> = {
  control: ({ c }) => c.overview(),
  sessions: ({ c, args }) => (args[0] ? c.session(args[0]) : c.sessions()),
  cancel: ({ c, args }) => c.cancel(required(args)),
  recover: ({ c, args, flags }) => c.recover(required(args), flags.replace === true),
  members,
  stop: ({ c, args, flags }) => c.stop(required(args), flags.force === true),
  takeover,
  release: takeover,
  resume: takeover,
  events: ({ c, flags }) => c.events(Number(flagStr(flags, 'after') || 0)),
  project: ({ c, args }) => c.settings(JSON.parse(required(args))),
  credential,
  webhook,
};

/**
 * Whether a command belongs to the control plane. Plain `oya stop <id...>|--all`
 * keeps its bulk meaning; only --force needs the durable endpoint.
 */
export function isControlCommand(command: string, flags: Flags): boolean {
  if (command === 'stop') return flags.force === true;
  return Object.hasOwn(COMMANDS, command);
}

/** Runs a control command and prints its answer as JSON. */
export async function runControl(command: string, args: string[], flags: Flags): Promise<void> {
  const c = client(flags).control;
  printJson(await COMMANDS[command]({ c, command, args, flags }));
}
