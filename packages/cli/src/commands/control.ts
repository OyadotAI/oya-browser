/**
 * The durable control-plane commands (`oya control`, `sessions`, `takeover`,
 * `credential`, `webhook` …). Each prints the API's answer as JSON. They
 * dispatch through a command map, and so do their own subcommands.
 */
import type { ControlRole, Oya } from '@oya-ai/browser';
import { flagNum, flagStr, parseJson, type Flags } from '../args.ts';
import { usage } from '../errors.ts';
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

/** The argument at `i`, or a usage error naming what `command` needs there. */
function required(args: string[], i: number, command: string, what: string): string {
  if (!args[i]) throw usage(`oya ${command} needs ${what}: oya ${command} <${what.split(' ').pop()}>.`);
  return args[i];
}

/** `--role`, defaulting to operator, checked against the roles that exist. */
function roleFlag(flags: Flags): ControlRole {
  const role = flagStr(flags, 'role') || 'operator';
  if (!ROLES.includes(role)) throw usage(`--role must be viewer, operator or administrator, not "${role}".`);
  return role as ControlRole;
}

/** Runs the subcommand named by `sub` of `command`, or refuses naming the ones there are. */
function subcommand(table: Record<string, Handler>, sub: string, call: ControlCall, command: string): Promise<unknown> {
  if (!Object.hasOwn(table, sub))
    throw usage(`Unknown ${command} subcommand "${sub}". Use ${orList(Object.keys(table))}.`);
  return table[sub](call);
}

/** "a, b or c". */
const orList = (names: string[]) => names.join(', ').replace(/, ([^,]*)$/, ' or $1');

/** `oya members [invite|remove <user>]`. */
const members: Handler = (call) => {
  if (!call.args[0]) return call.c.members();
  const table: Record<string, Handler> = {
    remove: ({ c, args }) => c.removeMember(required(args, 1, 'members remove', 'a user id')),
    invite: ({ c, flags }) => c.inviteMember(roleFlag(flags)),
  };
  return subcommand(table, call.args[0], call, 'members');
};

/** `oya credential new|revoke`. */
const credential: Handler = (call) => {
  const table: Record<string, Handler> = {
    revoke: ({ c, args }) => c.revokeCredential(required(args, 1, 'credential revoke', 'a credential id')),
    new: ({ c, flags }) => c.createCredential({ role: roleFlag(flags), label: flagStr(flags, 'label') }),
  };
  return subcommand(table, required(call.args, 0, 'credential', 'new or revoke'), call, 'credential');
};

/** `oya webhook new|remove|replay`. */
const webhook: Handler = (call) => {
  const table: Record<string, Handler> = {
    new: ({ c, args }) => c.createWebhook(required(args, 1, 'webhook new', 'an https url')),
    remove: ({ c, args }) => c.removeWebhook(required(args, 1, 'webhook remove', 'a webhook id')),
    replay: ({ c, args }) => c.replayDelivery(required(args, 1, 'webhook replay', 'a delivery id')),
  };
  return subcommand(table, required(call.args, 0, 'webhook', 'new, remove or replay'), call, 'webhook');
};

/** `--replace [--ws-url ...]` as recover's options; a --ws-url without --replace is refused before anything is sent. */
function recoverOptions(flags: Flags) {
  const wsUrl = flagStr(flags, 'ws-url');
  if (wsUrl && flags.replace !== true)
    throw usage('--ws-url is only used with --replace; recovering in place keeps the endpoint it had.');
  return { replace: flags.replace === true, ...(wsUrl ? { wsUrl } : {}) };
}

/** `oya project`'s settings, parsed, or an invalid_json error that shows how to quote them. */
const projectSettings = (text: string) =>
  parseJson(
    text,
    'The settings for oya project',
    `Quote them: oya project '{"retentionDays":7}'.`,
    'are',
  ) as Parameters<ControlCall['c']['settings']>[0];

/** `takeover` acquires; `release` and `resume` are sent as named. */
const takeover: Handler = ({ c, command, args }) =>
  c.takeover(
    required(args, 0, command, 'a session id'),
    command === 'takeover' ? 'acquire' : (command as 'release' | 'resume'),
  );

/** Every control command by name; exported so the help can be checked against it. */
export const CONTROL_COMMANDS: Record<string, Handler> = {
  control: ({ c }) => c.overview(),
  sessions: ({ c, args }) => (args[0] ? c.session(args[0]) : c.sessions()),
  cancel: ({ c, args }) => c.cancel(required(args, 0, 'cancel', 'a session id')),
  recover: ({ c, args, flags }) => c.recover(required(args, 0, 'recover', 'a session id'), recoverOptions(flags)),
  members,
  stop: ({ c, args, flags }) => c.stop(required(args, 0, 'stop', 'a session id'), flags.force === true),
  takeover,
  release: takeover,
  resume: takeover,
  events: ({ c, flags }) => c.events(flagNum(flags, 'after') ?? 0),
  project: ({ c, args }) => c.settings(projectSettings(required(args, 0, 'project', 'settings as JSON'))),
  credential,
  webhook,
};

/**
 * Whether a command belongs to the control plane. Plain `oya stop <id...>|--all`
 * keeps its bulk meaning; only --force needs the durable endpoint.
 */
export function isControlCommand(command: string, flags: Flags): boolean {
  if (command === 'stop') return flags.force === true;
  return Object.hasOwn(CONTROL_COMMANDS, command);
}

/** Runs a control command and prints its answer as JSON. */
export async function runControl(command: string, args: string[], flags: Flags): Promise<void> {
  const c = client(flags).control;
  printJson(await CONTROL_COMMANDS[command]({ c, command, args, flags }));
}
