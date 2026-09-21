/**
 * Control-plane test helpers: scratch SQLite stores and services, shortcuts
 * for putting a session into the state a test needs, and fake command-line
 * tools (docker, kubectl) on PATH.
 */
import { mkdtempSync, writeFileSync, chmodSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ControlStore } from '../../../src/modules/control/store.ts';
import { ControlService } from '../../../src/modules/control/service.ts';

/** A fresh directory under the OS temp dir. */
export const scratchDir = (prefix = 'oya-control-') => mkdtempSync(join(tmpdir(), prefix));

/** A ControlStore on its own SQLite file, without the single-writer lock file. */
export const scratchStore = () => new ControlStore({ path: join(scratchDir(), 'control.sqlite'), lock: false });

/** A ControlService over a scratch store. */
export const scratchService = () => new ControlService(scratchStore());

/** A cdp session admitted and marked ready on this replica. */
export async function readySession(service, key, id, options: any = {}) {
  await service.reserve(key, { id, provider: 'cdp', ...options });
  return service.complete(key, id, 200, {});
}

/** Merges `changes` into a stored row, bypassing the service's rules. */
export function patchRow(service, kind, id, changes) {
  return service.store.transact(async (tx) => {
    Object.assign(await tx.get(kind, id), changes);
  });
}

/** Writes a row as-is. */
export function putRow(service, kind, id, body) {
  return service.store.transact(async (tx) => {
    await tx.get(kind, id);
    tx.put(kind, id, body);
  });
}

/**
 * Installs a fake command-line tool named `name` on PATH. Each call is appended
 * to `log` as JSON ({ args, stdin }); the answer comes from `answer`, a function
 * source run against the args: `(args) => ({ stdout, stderr, code })`. Pass
 * `readStdin: false` for callers (execFile) that never close stdin.
 * Returns the log path and a restore function.
 */
export function fakeCli(name, answer: string, { readStdin = true } = {}) {
  const dir = scratchDir('oya-fake-cli-');
  const log = join(dir, 'calls.jsonl');
  const script = join(dir, name);
  writeFileSync(script, cliScript(log, answer, readStdin));
  chmodSync(script, 0o755);
  const path = process.env.PATH;
  process.env.PATH = `${dir}:${path}`;
  return { log, restore: () => void (process.env.PATH = path) };
}

/** The fake tool: a Node script that logs its call and prints the scripted answer. */
const cliScript = (log, answer, readStdin) => `#!/usr/bin/env node
const fs = require('node:fs');
const args = process.argv.slice(2);
let stdin = '';
if (${readStdin}) try { stdin = fs.readFileSync(0, 'utf8'); } catch {}
fs.appendFileSync(${JSON.stringify(log)}, JSON.stringify({ args, stdin }) + '\\n');
const r = (${answer})(args) || {};
if (r.stdout) process.stdout.write(r.stdout);
if (r.stderr) process.stderr.write(r.stderr);
process.exitCode = r.code || 0;
`;

/** The calls a fake tool recorded. */
export async function cliCalls(log) {
  const { readFile } = await import('node:fs/promises');
  const text = await readFile(log, 'utf8').catch(() => '');
  return text
    .split('\n')
    .filter(Boolean)
    .map((line) => JSON.parse(line));
}
