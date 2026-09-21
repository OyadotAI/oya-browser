/**
 * oya, the command line for the Oya browser control plane.
 *
 *   oya install                stand up a self-hosted control plane
 *   oya login                  save an API key
 *   oya init                   onboarding: model, provider, sign-in
 *   oya start                  start a browser
 *   oya goto <url>             navigate the newest browser
 *   oya ls                     what is running
 *   oya rm <id|--all>          stop browsers
 *   oya personas               identities and their concurrency
 *   oya open                   watch a browser work
 *   oya stealth-test           score this deployment against bot detectors
 *
 * The entry point: parse argv, run the command, explain a failure. The
 * commands themselves are in commands/.
 */

import type { OyaError } from '@oya-ai/browser';
import { parse } from './args.ts';
import { dispatch } from './commands/index.ts';
import { closePrompts } from './prompt.ts';
import { ARGV_SKIP, Status } from './constants.ts';

/** Prints a failure, with a hint for the statuses people hit most, and exits non-zero. */
function fail(err: unknown): never {
  const error = err as OyaError;
  console.error(`\n✗ ${error.message}`);
  if (error.status === Status.UNAUTHORIZED) console.error('  The API key was rejected. Run `oya login`.');
  if (error.status === Status.TOO_MANY_REQUESTS) {
    console.error('  A quota or a persona concurrency cap. `oya personas` shows what is running.');
  }
  process.exit(1);
}

const { command, args, flags } = parse(process.argv.slice(ARGV_SKIP));

try {
  await dispatch(command, args, flags);
} catch (err) {
  fail(err);
} finally {
  // An open readline holds the event loop open, so the process would never exit.
  closePrompts();
}
