/**
 * `oya install` — the promises that matter if the logic breaks.
 *
 * Driven through the built CLI against a throwaway checkout, because the risk
 * here is not a wrong string: it is writing a secret somewhere it should not go,
 * or overwriting a .env whose OYA_PROFILE_SECRET is the only key to every
 * credential already stored.
 */

import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import assert from 'node:assert/strict';

const CLI = new URL('../../dist/index.js', import.meta.url).pathname;

/** Enough of a checkout that findRepoRoot() accepts it. Nothing here is run. */
function fakeRepo() {
  const root = mkdtempSync(join(tmpdir(), 'oya-install-'));
  mkdirSync(join(root, 'server', 'src'), { recursive: true });
  writeFileSync(join(root, 'docker-compose.yml'), 'services: {}\n');
  writeFileSync(join(root, 'server', 'src', 'index.ts'), '// stub\n');
  return root;
}

const plan = {
  version: 1,
  host: 'docker',
  database: 'sqlite',
  fleet: 'docker-workers',
  workers: 3,
  llm: { provider: 'openai', baseUrl: 'https://api.openai.com/v1', model: 'gpt-4o-mini' },
  publicUrl: 'http://localhost:3100',
  optional: { captcha: '', recordingBucket: '', metrics: false },
};

function install(root, args, env = {}) {
  return execFileSync('node', [CLI, 'install', ...args], {
    cwd: root,
    encoding: 'utf8',
    stdio: ['pipe', 'pipe', 'pipe'],
    env: { ...process.env, ...env },
  });
}

function writePlan(root) {
  const path = join(root, 'plan.json');
  writeFileSync(path, JSON.stringify(plan));
  return path;
}

// ── 1. A replayed plan needs no input, and a dry run writes nothing ──────────
{
  const root = fakeRepo();
  const out = install(root, ['--config', writePlan(root), '--dry-run']);

  assert.equal(existsSync(join(root, '.env')), false, 'dry run must not write .env');
  assert.equal(existsSync(join(root, 'oya-install.json')), false, 'dry run must not write the answers file');
  assert.match(out, /control\s+docker · sqlite/, 'plan must reflect the replayed control plane');
  assert.match(out, /browsers\s+docker-workers ×3/, 'plan must reflect the replayed fleet');
  assert.match(out, /--dry-run: nothing was written/);
  console.log('✔ replayed plan is non-interactive and a dry run writes nothing');
}

// ── 2. Generated secrets never reach stdout ─────────────────────────────────
{
  const root = fakeRepo();
  const out = install(root, ['--config', writePlan(root), '--dry-run'], {
    OPENAI_API_KEY: 'sk-test-DO-NOT-PRINT-0123456789',
  });

  assert.ok(!out.includes('sk-test-DO-NOT-PRINT'), 'a supplied credential must never be echoed');
  assert.match(out, /OYA_PROFILE_SECRET=.*hidden/, 'generated secrets must be masked, not printed');
  assert.match(out, /API_KEYS=.*hidden/);
  // 32 random bytes as base64url is 43 chars; none of that may appear in output.
  const leak = /(?:^|[^A-Za-z0-9_-])[A-Za-z0-9_-]{43}(?:[^A-Za-z0-9_-]|$)/.exec(out);
  assert.equal(leak, null, `looks like a raw token leaked to stdout: ${leak?.[0]}`);
  console.log('✔ no secret material reaches stdout');
}

// ── 2b. Credentials that are not named "key"/"secret"/"token" are still hidden ──
{
  const root = fakeRepo();
  const pgPlan = { ...plan, database: 'postgres', fleet: 'cdp', workers: 0 };
  const path = join(root, 'pg.json');
  writeFileSync(path, JSON.stringify(pgPlan));

  const out = install(root, ['--config', path, '--dry-run'], {
    DATABASE_URL: 'postgres://user:hunter2@db.example.com:5432/oya',
    OYA_CDP_WS_URL: 'wss://private-endpoint.example.com/session',
  });

  // A connection string carries a password but matches no keyword pattern, which
  // is exactly why masking is an explicit list rather than a regex on the name.
  assert.ok(!out.includes('hunter2'), 'a database password must never be printed');
  assert.ok(!out.includes('db.example.com'), 'nor the host it belongs to');
  assert.ok(!out.includes('private-endpoint'), 'nor a CDP endpoint URL');
  assert.match(out, /DATABASE_URL=.*hidden/, 'it is shown as hidden, not omitted');
  console.log('✔ credentials without obvious names are still masked');
}

// ── 3. An existing .env is never clobbered ──────────────────────────────────
{
  const root = fakeRepo();
  const envPath = join(root, '.env');
  const original = 'OYA_PROFILE_SECRET=sentinel-kek-value\nAPI_KEYS=existing-key\n';
  writeFileSync(envPath, original, { mode: 0o600 });

  const out = install(root, ['--config', writePlan(root), '--dry-run']);

  assert.equal(readFileSync(envPath, 'utf8'), original, 'existing .env must be byte-identical after a dry run');
  assert.ok(!out.includes('sentinel-kek-value'), 'an existing KEK must not be echoed');
  assert.match(out, /\.env exists — keeping its OYA_PROFILE_SECRET/, 'must say it will reuse the existing KEK');
  assert.equal((statSync(envPath).mode & 0o777).toString(8), '600');
  console.log('✔ an existing .env is preserved and its KEK reused');
}

console.log('\nAll install wizard checks passed.');
