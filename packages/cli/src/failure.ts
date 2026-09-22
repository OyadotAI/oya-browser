/**
 * How a failed command ends: one line on stderr saying what went wrong (or one
 * JSON object with --json), a hint with the next step when there is one, and
 * an exit code a script can trust: 2 when nothing was sent because the command
 * itself was wrong, 1 for everything else. The exit code is set, not forced,
 * so output already written to a pipe is never cut short.
 */
import type { OyaError } from '@oya-ai/browser';
import { CliError } from './errors.ts';
import { origin as originOf, type Origin } from './config.ts';
import { ExitCode, Status } from './constants.ts';

/** Codes that mean nothing was sent: the command, not the server, was wrong. */
const USAGE_CODES = new Set(['usage', 'invalid_json']);

/** What a failure says, whatever raised it. */
interface Failure {
  /** The one-line message. */
  message: string;
  /** A code to branch on. */
  code: string;
  /** The HTTP status, or 0 when no HTTP answer came. */
  status: number;
  /** A second line with the next step. */
  hint?: string;
  /** The browser holding an endpoint, when a start was refused for one. */
  browserId?: string;
  /** True when the command already printed its own lines. */
  shown?: boolean;
}

/** Prints the failure and sets the exit code. `argv` is read again, since parsing itself may be what failed. */
export function fail(err: unknown, argv: string[]): void {
  const failure = describe(err, originOf(sourceFlags(argv)));
  process.exitCode = USAGE_CODES.has(failure.code) ? ExitCode.USAGE : ExitCode.FAILED;
  if (argv.includes('--json')) printJson(failure, err, debugging(argv));
  else printPlain(failure, err, debugging(argv));
}

/** Whether --debug (or OYA_DEBUG=1) asked for the status, the body and the stack. */
const debugging = (argv: string[]) => argv.includes('--debug') || process.env.OYA_DEBUG === '1';

/** --key and --url as typed, found without the parser so a parse error still names the right source. */
function sourceFlags(argv: string[]) {
  const valueOf = (name: string) => {
    const at = argv.findIndex((a) => a === `--${name}` || a.startsWith(`--${name}=`));
    if (at < 0) return undefined;
    return argv[at].includes('=') ? argv[at].split('=').slice(1).join('=') : argv[at + 1];
  };
  return { apiKey: valueOf('key'), baseUrl: valueOf('url') };
}

/** One failure of any kind, in the shape both outputs print. */
function describe(err: unknown, origin: Origin): Failure {
  if (err instanceof CliError)
    return { message: err.message, code: err.code, status: 0, hint: err.hint, shown: err.shown };
  const e = err as OyaError;
  if (typeof e?.status !== 'number')
    return { message: String((err as Error)?.message ?? err), code: 'internal', status: 0 };
  return fromServer(e, origin);
}

/** A failed call: the CLI's own line for an unreachable server or a rejected key, else the server's message. */
function fromServer(e: OyaError, origin: Origin): Failure {
  const body = (e.body ?? {}) as Record<string, unknown>;
  const code = typeof body.code === 'string' ? body.code : e.status === 0 ? 'unreachable' : 'http_error';
  const base = { code, status: e.status, browserId: typeof body.browserId === 'string' ? body.browserId : undefined };
  if (e.status === 0 && code === 'unreachable') return { ...base, ...unreachable(origin, body.cause) };
  if (isInvalidKey(e, body)) return { ...base, ...invalidKey(origin) };
  return { ...base, message: renamed(e.message, body, origin), hint: hintFor(e.status, code, base.browserId) };
}

/** The SDK names its own option ("baseUrl"); the CLI user typed --url or set OYA_BASE_URL, so say that. */
function renamed(message: string, body: Record<string, unknown>, origin: Origin): string {
  if (body.field !== 'baseUrl' || !message.startsWith('baseUrl ')) return message;
  const source = origin.baseUrlFrom === 'saved' ? 'The URL saved by oya login' : origin.baseUrlFrom;
  return `${source}${message.slice('baseUrl'.length)}`;
}

/** A key the server rejected: 401, or the 403 its auth answers for an unknown key. */
const isInvalidKey = (e: OyaError, body: Record<string, unknown>) =>
  e.status === Status.UNAUTHORIZED || (e.status === Status.FORBIDDEN && body.error === 'Invalid API key');

/** The system's reason, in words. */
const CAUSES: Record<string, string> = { ECONNREFUSED: 'nothing is listening there', ENOTFOUND: 'no such host' };

/** Where an address came from, and what to do about each. */
const URL_HINTS: Record<string, string> = {
  '--url': 'The URL came from --url. Start the server there, or pass the right --url.',
  OYA_BASE_URL: 'The URL came from OYA_BASE_URL. Start the server there, or export the right URL.',
  saved: 'The URL was saved by oya login. Start the server there, or run oya login --url <url>.',
  default: 'That is the hosted default. Check your connection, or pass --url for a self-hosted server.',
};

/** Nothing answered at the address: say which address, why, and where it came from. */
function unreachable(origin: Origin, cause: unknown) {
  const why = typeof cause === 'string' ? (Object.hasOwn(CAUSES, cause) ? CAUSES[cause] : cause) : 'no answer';
  return { message: `Could not reach ${origin.baseUrl}: ${why}.`, hint: URL_HINTS[origin.baseUrlFrom] };
}

/** Where a key came from, as the message names it. */
const KEY_NAMES: Record<string, string> = {
  '--key': '--key',
  OYA_API_KEY: 'OYA_API_KEY',
  saved: 'oya login',
  none: 'nowhere',
};

/** A rejected key: which address refused it, where the key came from, and what to do. */
function invalidKey(origin: Origin) {
  const message = `Invalid API key for ${origin.baseUrl} (the key came from ${KEY_NAMES[origin.apiKeyFrom]}).`;
  const overrides = origin.apiKeyFrom === 'OYA_API_KEY' && origin.savedKey;
  const hint = overrides
    ? 'OYA_API_KEY overrides the key saved by oya login. Unset it, or export the right key.'
    : 'Run oya login to save a new key.';
  return { message, hint };
}

/** The next step for the other failures people hit most; none when the message stands alone. */
function hintFor(status: number, code: string, holder?: string): string | undefined {
  if (status === Status.TOO_MANY_REQUESTS)
    return 'A quota or a persona concurrency cap. oya personas shows what is running.';
  if (holder) return `oya stop ${holder} frees it; oya goto <url> --id ${holder} drives it.`;
  if (code === 'replacement_endpoint_required')
    return 'Add --ws-url ws://host:9222, the Chrome the replacement runs on.';
  return undefined;
}

/** `✗ message`, an indented hint, and with --debug the status, code, body and stack. */
function printPlain(failure: Failure, err: unknown, debug: boolean): void {
  if (!failure.shown) console.error(`✗ ${failure.message}`);
  if (failure.hint && !failure.shown) console.error(`  ${failure.hint}`);
  if (!debug) return;
  console.error(`  status ${failure.status} · code ${failure.code}`);
  if ((err as OyaError)?.body !== undefined) console.error(JSON.stringify((err as OyaError).body));
  if ((err as Error)?.stack) console.error((err as Error).stack);
}

/** One JSON line on stderr, with the stack when --debug asked for it. */
function printJson(failure: Failure, err: unknown, debug: boolean): void {
  const { message, code, status, browserId } = failure;
  const stack = debug ? { stack: (err as Error)?.stack } : {};
  console.error(JSON.stringify({ error: message, code, status, ...(browserId ? { browserId } : {}), ...stack }));
}
