/**
 * Finding the oya-browser checkout. The wizard drives this repo's compose file
 * and manifests, so it needs one, and offers to clone it when there is none.
 */
import { existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { ask } from '../prompt.ts';
import { run } from './shell.ts';
import { REPO_SEARCH_DEPTH } from './constants.ts';

/** The server entry point: TypeScript now, JavaScript in checkouts from before the move. */
const SERVER_ENTRIES = ['index.ts', 'index.js'];

/** Whether a directory is the checkout's root. */
const isRepoRoot = (dir: string) =>
  existsSync(join(dir, 'docker-compose.yml')) &&
  SERVER_ENTRIES.some((entry) => existsSync(join(dir, 'server', 'src', entry)));

/** The wizard drives this repo's compose file and manifests, so it needs the checkout. */
export function findRepoRoot(from = process.cwd()): string | null {
  let dir = from;
  for (let i = 0; i < REPO_SEARCH_DEPTH; i++) {
    if (isRepoRoot(dir)) return dir;
    const up = dirname(dir);
    if (up === dir) break;
    dir = up;
  }
  return null;
}

/** The checkout: found above the working directory, or cloned where the user says. */
export async function locateRepo(): Promise<string> {
  const found = findRepoRoot();
  if (found) return found;
  console.log('\nThis does not look like an oya-browser checkout.');
  const where = await ask('Clone it to:', join(process.cwd(), 'oya-browser'));
  if (existsSync(where)) throw new Error(`${where} already exists`);
  const repo = 'https://github.com/OyadotAI/oya-browser.git';
  await run('git', ['clone', '--depth', '1', repo, where], { cwd: process.cwd() });
  return where;
}
