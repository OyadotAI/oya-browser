#!/usr/bin/env node
/**
 * Apply the SQL migrations. Until now this was a manual copy-paste into the
 * Supabase SQL editor, which is why a self-host install could not be scripted.
 *
 *   DATABASE_URL=postgres://... node server/migrations/run.mjs [--dry-run]
 *
 * Shells psql rather than taking a driver dependency, which is both the house
 * pattern (see control/managed.js) and exactly what docs/control-plane.md already
 * tells operators to do by hand.
 *
 * Idempotent: every applied file is recorded in public.schema_migrations, and each
 * file is applied in a single transaction together with its own bookkeeping row,
 * so a failure leaves neither a half-applied schema nor a false record of it.
 *
 * SQLite needs nothing here, SqliteBackend creates its own schema on first open.
 */

import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { readdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const exec = promisify(execFile);
const HERE = dirname(fileURLToPath(import.meta.url));
// Migrations only plain Postgres needs, standing in for what Supabase provides.
const PLAIN = join(HERE, 'postgres');
// Accounts, profiles and RLS keyed on auth.users / auth.uid(): Supabase only.
const AUTH_ONLY = /^(001|003|009|010)_/;
const url = process.env.DATABASE_URL || '';
const dryRun = process.argv.includes('--dry-run');

async function psql(args) {
  const { stdout } = await exec('psql', [url, '-v', 'ON_ERROR_STOP=1', ...args], {
    maxBuffer: 8 * 1024 * 1024,
    // A connection string on the command line is visible in `ps`; psql reads this instead.
    env: { ...process.env, PGCONNECT_TIMEOUT: '10' },
  });
  return stdout.trim();
}

const query = (sql) => psql(['-Atqc', sql]);

async function main() {
  if (!url) {
    console.log('[migrate] DATABASE_URL is not set.');
    console.log('[migrate] SQLite deployments need no migrations, the control store');
    console.log('[migrate] creates its own schema. Nothing to do.');
    return;
  }

  try {
    await query('select 1');
  } catch (err) {
    throw new Error(`cannot reach the database: ${err.stderr?.trim() || err.message}`);
  }

  // Supabase supplies auth.users and service_role; plain Postgres does not, and
  // the AUTH_ONLY files are written against them. The presence of auth.users is the tell.
  const hosted = (await query("select to_regclass('auth.users') is not null")) === 't';
  console.log(`[migrate] target: ${hosted ? 'Supabase (auth schema present)' : 'plain Postgres'}`);

  const sqlIn = (dir) =>
    readdirSync(dir)
      .filter((f) => f.endsWith('.sql'))
      .sort();
  const files = [];
  // What Supabase would have provided (the schema, roles, api_keys) comes first.
  if (!hosted) for (const name of sqlIn(PLAIN)) files.push([`postgres/${name}`, join(PLAIN, name)]);
  for (const name of sqlIn(HERE)) {
    if (!hosted && AUTH_ONLY.test(name)) continue;
    files.push([name, join(HERE, name)]);
  }

  await psql([
    '-c',
    'create table if not exists public.schema_migrations (filename text primary key, applied_at timestamptz not null default now())',
  ]);
  const applied = new Set((await query('select filename from public.schema_migrations')).split('\n').filter(Boolean));

  const todo = files.filter(([name]) => !applied.has(name));
  if (!todo.length) {
    console.log(`[migrate] up to date (${applied.size} already applied).`);
    return;
  }

  if (dryRun) {
    for (const [name] of todo) console.log(`[migrate] would apply ${name}`);
    console.log(`[migrate] ${todo.length} pending; nothing was written.`);
    return;
  }

  for (const [name, path] of todo) {
    process.stdout.write(`[migrate] ${name} … `);
    // One transaction for the file and the record of it, so they cannot disagree.
    await psql([
      '--single-transaction',
      '-f',
      path,
      '-c',
      `insert into public.schema_migrations (filename) values ('${name.replace(/'/g, "''")}')`,
    ]);
    console.log('ok');
  }
  console.log(`[migrate] applied ${todo.length} migration(s).`);
}

main().catch((err) => {
  console.error(`\n[migrate] failed: ${err.stderr?.trim() || err.message}`);
  process.exit(1);
});
