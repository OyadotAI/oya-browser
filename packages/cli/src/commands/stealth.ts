/**
 * `oya stealth-test`. The stealth harness lives with the server, because it
 * needs a browser to measure. This just runs it where it is.
 */
import { spawn } from 'node:child_process';
import type { Flags } from '../args.ts';

/** Where the harness lives inside a checkout of the server. */
const HARNESS = 'tests/integration/stealth.test.js';

/** Starts the harness in the server folder. */
const spawnHarness = (flags: Flags) =>
  spawn('node', [HARNESS, ...(flags.live ? ['--live'] : [])], {
    cwd: process.env.OYA_SERVER_DIR || 'server',
    stdio: 'inherit',
  });

/** Runs the server's stealth harness, `--live` passed through. */
export function cmdStealthTest(flags: Flags): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawnHarness(flags);
    const missing = 'Could not find the stealth harness. Run it from a checkout, or set OYA_SERVER_DIR.';
    child.on('error', () => reject(new Error(missing)));
    child.on('exit', (code) => (code === 0 ? resolve() : reject(new Error(`stealth test exited ${code}`))));
  });
}
