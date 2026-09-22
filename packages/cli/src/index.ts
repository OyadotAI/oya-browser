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
 * The entry point: parse argv, run the command, explain a failure. Parsing is
 * inside the same try, so an unknown flag is explained like any other
 * failure. The commands themselves are in commands/.
 */

import { parse } from './args.ts';
import { dispatch } from './commands/index.ts';
import { fail } from './failure.ts';
import { closePrompts } from './prompt.ts';
import { ARGV_SKIP } from './constants.ts';

const argv = process.argv.slice(ARGV_SKIP);

try {
  const { command, args, flags } = parse(argv);
  await dispatch(command, args, flags);
} catch (err) {
  fail(err, argv);
} finally {
  // An open readline holds the event loop open, so the process would never exit.
  closePrompts();
}
