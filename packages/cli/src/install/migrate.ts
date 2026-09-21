/**
 * Applying the database schema. It has to exist before the server opens a
 * connection to it, so this runs the repo's migration script first.
 */
import { spawn } from 'node:child_process';
import { join } from 'node:path';
import { spinner } from '../prompt.ts';

/** Starts the migration script with DATABASE_URL set, stderr piped. */
const startMigrations = (root: string, databaseUrl: string) =>
  spawn(process.execPath, [join(root, 'server', 'migrations', 'run.mjs')], {
    cwd: root,
    stdio: ['ignore', 'pipe', 'pipe'],
    env: { ...process.env, DATABASE_URL: databaseUrl },
  });

/** Runs server/migrations/run.mjs against `databaseUrl`; rejects with its stderr. */
function runMigrations(root: string, databaseUrl: string): Promise<void> {
  return new Promise<void>((resolve, reject) => {
    const child = startMigrations(root, databaseUrl);
    let err = '';
    child.stderr.on('data', (d) => (err += d));
    child.on('error', reject);
    const failure = (code: number | null) => new Error(err.trim() || `migrations exited ${code}`);
    child.on('close', (code) => (code === 0 ? resolve() : reject(failure(code))));
  });
}

/** How to retry after a failed migration. */
const RETRY = 'Fix the database, then re-run: DATABASE_URL=… make migrate';

/** Applies migrations with a spinner; a failure says how to retry. */
export async function migrate(root: string, databaseUrl: string): Promise<void> {
  const spin = spinner('applying migrations');
  try {
    await runMigrations(root, databaseUrl);
    spin.done();
  } catch (e) {
    spin.fail();
    throw new Error(`Migrations failed: ${(e as Error).message}\n  ${RETRY}`);
  }
}
